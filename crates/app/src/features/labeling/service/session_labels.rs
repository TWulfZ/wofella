//! Session answers about a played map's dominant pattern, and the labelling progress page
//! (ADR 0020). Kept apart from the gold protocol: nothing here reaches `gold_labels`.

use std::collections::{BTreeMap, BTreeSet};

use wolluf_core::{ChartMd5, PlayId, ProfileId, UnixUs};
use wolluf_engine::taxonomy;
use wolluf_store::repo::labels::{
    DominantAnswer, GoldAnswer, NewPlayLabel, NewUndo, PlayLabel, append_play_label,
    append_play_label_undo, play_labels,
};
use wolluf_store::time::format_rfc3339_ms;

use super::{
    APP_VERSION, LabelingService, count, counts, labelled_profile, parse_md5, patterns_of,
};
use crate::context::blocking_join_error;
use crate::errors::AppError;
use crate::features::labeling::dto::{
    DayCountDto, LabelEventDto, LabelProgressDto, RecentLabelDto, SessionLabelDto,
    SessionLabelSubmitDto,
};
use crate::features::labeling::keys;

const US_PER_MIN: i64 = 60_000_000;
const US_PER_DAY: i64 = 86_400_000_000;
/// `YYYY-MM-DD` of an RFC 3339 timestamp.
const DATE_LEN: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ProgressParams {
    pub(super) days: u16,
    pub(super) recent: usize,
    /// Civil offsets span UTC-12 to UTC+14.
    pub(super) max_utc_offset_min: u16,
}

impl Default for ProgressParams {
    fn default() -> Self {
        Self {
            days: 30,
            recent: 20,
            max_utc_offset_min: 14 * 60,
        }
    }
}

impl LabelingService<'_> {
    /// Appends one `play_label` event under the self profile. `NOT_FOUND` for a chart the
    /// catalog does not list under `req.keymode`; `INVALID_INPUT` for an unknown pattern or a
    /// play that is not a self play of that chart (ADR 0005).
    pub async fn session_submit(
        &self,
        req: SessionLabelSubmitDto,
    ) -> Result<LabelEventDto, AppError> {
        let profile = labelled_profile(req.keymode)?;
        let keymode = profile.keymode;
        let md5 = parse_md5(&req.md5)?;
        let answer = match req.pattern.as_deref() {
            None => DominantAnswer::NoPattern,
            Some(p) => DominantAnswer::Pattern(
                taxonomy::by_id(profile.taxonomy, p)
                    .map(|def| def.id.clone())
                    .ok_or_else(|| {
                        AppError::invalid_input()
                            .with_key(keys::UNKNOWN_PATTERN)
                            .with_arg("pattern", p)
                    })?,
            ),
        };
        let play_id = req
            .play_id
            .as_deref()
            .map(|p| {
                p.parse::<PlayId>()
                    .map_err(|_| AppError::invalid_input().with_arg("playId", p))
            })
            .transpose()?;
        let listed = self
            .ctx
            .library()
            .catalog_charts(vec![md5])
            .await?
            .remove(&md5)
            .is_some_and(|c| c.keymode == keymode.columns());
        if !listed {
            return Err(AppError::not_found().with_arg("md5", req.md5.clone()));
        }
        let me = self.require_self().await?;
        if let Some(id) = play_id {
            let aliases = self.ctx.players().self_alias_ids().await?;
            let found = self.ctx.plays().get(id).await?;
            if !found.is_some_and(|p| p.chart_md5 == md5 && aliases.contains(&p.alias_id)) {
                return Err(AppError::invalid_input().with_arg("playId", id.to_string()));
            }
        }
        let now = self.ctx.clock().now();
        let id = self
            .append(now, move |tx, id| {
                append_play_label(
                    tx,
                    &NewPlayLabel {
                        id,
                        ts: now,
                        profile_id: me,
                        chart_md5: md5,
                        keymode,
                        play_id,
                        answer,
                        app_version: APP_VERSION.to_owned(),
                    },
                )
            })
            .await?;
        Ok(LabelEventDto { id: id.to_string() })
    }

    /// Appends an `undo` of one of the self profile's session answers; a gold label id is
    /// `NOT_FOUND` here.
    pub async fn session_undo(&self, event_id: &str) -> Result<(), AppError> {
        let target = ulid::Ulid::from_string(event_id)
            .map_err(|_| AppError::invalid_input().with_arg("eventId", event_id))?;
        let me = self.require_self().await?;
        let now = self.ctx.clock().now();
        self.append(now, move |tx, id| {
            append_play_label_undo(
                tx,
                &NewUndo {
                    id,
                    ts: now,
                    profile_id: me,
                    target,
                    app_version: APP_VERSION.to_owned(),
                },
            )
        })
        .await?;
        Ok(())
    }

    /// Each chart's effective session answer, by md5; empty without a self profile.
    pub async fn session_label_states(
        &self,
    ) -> Result<BTreeMap<String, SessionLabelDto>, AppError> {
        let Some(me) = self.self_profile().await? else {
            return Ok(BTreeMap::new());
        };
        Ok(latest_per_chart(self.play_labels(me).await?)
            .into_iter()
            .map(|(md5, l)| (md5.to_string(), session_label_dto(&l)))
            .collect())
    }

    /// Gold and session labelling of `keymode`, with days cut at `utc_offset_min` east of UTC
    /// (the UI's local offset).
    pub async fn progress(
        &self,
        keymode: u8,
        utc_offset_min: i16,
    ) -> Result<LabelProgressDto, AppError> {
        let params = ProgressParams::default();
        let profile = labelled_profile(keymode)?;
        if utc_offset_min.unsigned_abs() > params.max_utc_offset_min {
            return Err(
                AppError::invalid_input().with_arg("utcOffsetMin", utc_offset_min.to_string())
            );
        }
        let (mut gold, session) = match self.self_profile().await? {
            Some(me) => (self.labels(me).await?, self.play_labels(me).await?),
            None => (Vec::new(), Vec::new()),
        };
        gold.retain(|l| l.keymode == profile.keymode);
        // Contributions, not answers (ADR 0020): a relabelled map counts once, on the day of its
        // effective answer, so the daily series adds up to the total.
        let session = latest_per_chart(
            session
                .into_iter()
                .filter(|l| l.keymode == profile.keymode)
                .collect(),
        );

        let mut per_pattern = BTreeMap::new();
        let mut per_axis = BTreeMap::new();
        for l in &gold {
            let patterns = patterns_of(l);
            let axes: BTreeSet<String> = patterns
                .iter()
                .filter_map(|p| taxonomy::by_id(profile.taxonomy, p.as_str()))
                .map(|d| d.axis.to_string())
                .collect();
            for p in patterns {
                *per_pattern.entry(p.to_string()).or_insert(0) += 1;
            }
            for axis in axes {
                *per_axis.entry(axis).or_insert(0) += 1;
            }
        }

        let offset = i64::from(utc_offset_min) * US_PER_MIN;
        let day_of = |ts: UnixUs| (ts.0 + offset).div_euclid(US_PER_DAY);
        let today = day_of(self.ctx.clock().now());
        let first = today - i64::from(params.days) + 1;
        let mut per_day: Vec<DayCountDto> = (first..=today)
            .map(|day| DayCountDto {
                day: format_rfc3339_ms(UnixUs(day * US_PER_DAY))[..DATE_LEN].to_owned(),
                gold: 0,
                session: 0,
            })
            .collect();
        let index = |ts: UnixUs| usize::try_from(day_of(ts) - first).ok();
        for l in &gold {
            if let Some(d) = index(l.ts).and_then(|i| per_day.get_mut(i)) {
                d.gold += 1;
            }
        }
        for l in session.values() {
            if let Some(d) = index(l.ts).and_then(|i| per_day.get_mut(i)) {
                d.session += 1;
            }
        }

        let gold_total = count(gold.len());
        let gold_no_pattern = count(
            gold.iter()
                .filter(|l| l.answer == GoldAnswer::NoPattern)
                .count(),
        );
        gold.sort_by_key(|l| std::cmp::Reverse(l.id));
        gold.truncate(params.recent);
        let catalog = self
            .ctx
            .library()
            .catalog_charts(gold.iter().map(|l| l.anchor.chart_md5()).collect())
            .await?;
        let recent = gold
            .iter()
            .map(|l| {
                let md5 = l.anchor.chart_md5();
                let chart = catalog.get(&md5);
                RecentLabelDto {
                    event_id: l.id.to_string(),
                    md5: md5.to_string(),
                    title: chart.map(|c| c.title.clone()),
                    version: chart.map(|c| c.version.clone()),
                    patterns: patterns_of(l).iter().map(ToString::to_string).collect(),
                    no_pattern: l.answer == GoldAnswer::NoPattern,
                    at: format_rfc3339_ms(l.ts),
                }
            })
            .collect();

        Ok(LabelProgressDto {
            gold_total,
            gold_no_pattern,
            per_pattern: counts(per_pattern),
            per_axis: counts(per_axis),
            session_labels: count(session.len()),
            per_day,
            recent,
        })
    }

    async fn play_labels(&self, me: ProfileId) -> Result<Vec<PlayLabel>, AppError> {
        let user = self.ctx.user_db().clone();
        let labels = tokio::task::spawn_blocking(move || user.read(|c| play_labels(c, me)))
            .await
            .map_err(blocking_join_error)??;
        Ok(labels)
    }
}

/// `labels` come in append order, so the last one per chart is its effective answer.
fn latest_per_chart(labels: Vec<PlayLabel>) -> BTreeMap<ChartMd5, PlayLabel> {
    labels.into_iter().map(|l| (l.chart_md5, l)).collect()
}

fn session_label_dto(l: &PlayLabel) -> SessionLabelDto {
    SessionLabelDto {
        event_id: l.id.to_string(),
        pattern: match &l.answer {
            DominantAnswer::Pattern(p) => Some(p.to_string()),
            DominantAnswer::NoPattern => None,
        },
        at: format_rfc3339_ms(l.ts),
    }
}
