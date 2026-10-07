//! `LabelingService`: the labelling feature's only entry point for shells (D12). Labels are
//! blind: nothing here consults the pattern engine, so the gold set stays an unbiased test set
//! for it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use wolluf_core::{
    ChartMd5, ColMask, FileTime, Keymode, PatternId, ProfileId, SegmentAnchor, TimeUs, UnixUs,
};
use wolluf_engine::examples;
use wolluf_engine::labels::source as label_source;
use wolluf_engine::profile::{KeymodeProfile, Registry};
use wolluf_engine::taxonomy;
use wolluf_engine::window::chart_window;
use wolluf_source_osu::probe::{OsuProcessProbe, ProbeParams, parse_osu_title, system_probe};
use wolluf_source_osu::replay_dir::{self, replay_player};
use wolluf_store::repo::labels::{
    GoldAnswer, GoldLabel, NewGoldLabel, NewUndo, ThumbPref, append_gold_label, append_undo,
    gold_labels,
};
use wolluf_store::repo::ledger::feedback_event;
use wolluf_store::time::format_rfc3339_ms;

use wolluf_core::ErrorCode;
use wolluf_store::StoreError;

use super::dto::{
    AnchorDto, ChartTimelineDto, ChartTimelineRequestDto, CountDto, LabelEventDto, LabelExportDto,
    LabelStatsDto, LabelSubmitDto, LabelWindowDto, MoveWindowRequestDto, NowPlayingDto,
    NowPlayingRequestDto, NowPlayingSourceDto, PatternDefDto, PatternExampleDto, RandomRequestDto,
    SampleRequestDto, SpanDto, ThumbPrefDto, WindowAtRequestDto, WindowOpDto,
};
use super::keys;
use super::now_playing::{NowPlayingParams, title_matches};
use super::sampler::{self, Assignment, ChartFacts, LevelFilter, LevelLabel, Pool, SamplerParams};
use super::window::{self, Reshape};
use crate::context::{AppContext, blocking_join_error, catalog_install};
use crate::errors::AppError;
use crate::features::library::dto::{ChartWindowDto, LibraryChartDto};
use crate::jobs::to_system_time;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const US_PER_MS: i64 = 1_000;
const UNKNOWN_STRATUM: &str = "unknown";
const FLAG_MIXED: &str = "mixed";
const FLAG_UNSURE: &str = "unsure";
/// Examples are not library charts; zeros keep the window DTO without naming one.
const SYNTHETIC_MD5: ChartMd5 = ChartMd5([0; 16]);
/// Fixed, so the window depends only on the chart and the session's shown windows.
const NOW_PLAYING_SEED: u64 = 0;

pub struct LabelingService<'a> {
    ctx: &'a AppContext,
    params: SamplerParams,
    now_playing: NowPlayingParams,
    probe: Arc<dyn OsuProcessProbe>,
}

/// The keymode's parsed charts with their strata. Strata come from every chart, never a
/// filtered subset, or their names would shift with the filter.
struct Catalog {
    charts: Vec<LibraryChartDto>,
    played: BTreeSet<ChartMd5>,
    facts: Vec<ChartFacts>,
    assigned: BTreeMap<ChartMd5, Assignment>,
}

impl Catalog {
    fn window(
        &self,
        md5: ChartMd5,
        t0: TimeUs,
        t1: TimeUs,
        keymode: Keymode,
    ) -> Result<LabelWindowDto, AppError> {
        let key = md5.to_string();
        let chart = self
            .charts
            .iter()
            .find(|c| c.md5 == key)
            .ok_or_else(|| AppError::internal(format!("picked chart {key} is not listed")))?;
        let anchor = SegmentAnchor::new(md5, t0, t1, ColMask::full(keymode), keymode)
            .map_err(|e| AppError::internal(format!("picked anchor: {e}")))?;
        let assigned = self.assigned.get(&md5);
        Ok(LabelWindowDto {
            anchor: anchor_dto(&anchor)?,
            title: chart.title.clone(),
            artist: chart.artist.clone(),
            version: chart.version.clone(),
            creator: chart.creator.clone(),
            stars: chart.stars,
            level: assigned.and_then(|a| a.label.clone()),
            stratum: assigned.map_or_else(|| UNKNOWN_STRATUM.to_owned(), |a| a.stratum.to_string()),
            played: self.played.contains(&md5),
        })
    }
}

/// One line of the gold-set export: anchors and pattern ids only, never map content or player
/// names (architecture §3 `fixtures/labels/`).
#[derive(Debug, Serialize)]
struct ExportRow {
    md5: String,
    t0_us: i64,
    t1_us: i64,
    /// 1-based, column 1 leftmost.
    cols: Vec<u8>,
    /// Empty exactly when `no_pattern`.
    patterns: Vec<String>,
    no_pattern: bool,
    flags: Vec<&'static str>,
    thumb_pref: Option<&'static str>,
    labelled_at: String,
}

impl<'a> LabelingService<'a> {
    pub fn new(ctx: &'a AppContext) -> Self {
        Self::with_probe(ctx, Arc::from(system_probe(ProbeParams::default())))
    }

    pub fn with_probe(ctx: &'a AppContext, probe: Arc<dyn OsuProcessProbe>) -> Self {
        Self {
            ctx,
            params: SamplerParams::default(),
            now_playing: NowPlayingParams::default(),
            probe,
        }
    }

    pub fn taxonomy(&self, keymode: u8) -> Result<Vec<PatternDefDto>, AppError> {
        Ok(labelled_profile(keymode)?
            .taxonomy
            .iter()
            .map(|p| PatternDefDto {
                id: p.id.to_string(),
                axis: p.axis.to_string(),
                key: p.key.to_owned(),
                description: p.description.to_owned(),
            })
            .collect())
    }

    /// One synthetic chart per pattern, so previews never show library content (ADR 0018).
    /// `layout_id` `None` draws with the stored preference; an id that is no preset of the
    /// keymode is `INVALID_INPUT`.
    pub async fn pattern_examples(
        &self,
        keymode: u8,
        layout_id: Option<&str>,
    ) -> Result<Vec<PatternExampleDto>, AppError> {
        let profile = labelled_profile(keymode)?;
        let layout = match layout_id {
            Some(id) => profile
                .layout_by_id(id)
                .ok_or_else(|| AppError::invalid_input().with_arg("layoutId", id))?,
            None => {
                let preferred = self.ctx.settings().hand_layout(keymode).await?;
                profile
                    .layout_by_id(&preferred)
                    .unwrap_or_else(|| profile.layout())
            }
        };
        Ok(examples::for_keymode(profile.keymode)
            .into_iter()
            .map(|ex| {
                let (from_ms, to_ms) = ex.display_ms;
                let window = chart_window(&ex.chart, &layout, from_ms, to_ms);
                PatternExampleDto {
                    id: ex.id.to_string(),
                    window: ChartWindowDto::from_window(SYNTHETIC_MD5, from_ms, to_ms, window),
                }
            })
            .collect())
    }

    /// The window for round `req.round`, or `None` when no chart has a free window left.
    pub async fn sample(&self, req: SampleRequestDto) -> Result<Option<LabelWindowDto>, AppError> {
        let keymode = labelled_profile(req.keymode)?.keymode;
        let seed = parse_seed(&req.seed)?;
        let window = self.window_len(req.window_ms)?;
        let exclude = self.exclude(keymode, &req.exclude).await?;
        let catalog = self.catalog(keymode).await?;
        let filter = LevelFilter {
            scale: req.scale,
            level_min: req.level_min,
            level_max: req.level_max,
        };
        let pool = Pool::new(seed, &catalog.facts, &catalog.assigned, &filter);
        let ctx = self.ctx;
        let rows_of = move |md5| async move { rows_or_none(ctx, md5).await };
        let Some(pick) = sampler::sample(
            seed,
            req.round,
            &pool,
            &exclude,
            window,
            &self.params,
            rows_of,
        )
        .await?
        else {
            return Ok(None);
        };
        catalog
            .window(pick.md5, pick.t0, pick.t1, keymode)
            .map(Some)
    }

    /// A free window inside `req.md5`. `NOT_FOUND` for a chart the keymode's catalog has not
    /// parsed, `CONFLICT` when every window of it overlaps `exclude` or a stored label.
    pub async fn window_at(&self, req: WindowAtRequestDto) -> Result<LabelWindowDto, AppError> {
        let keymode = labelled_profile(req.keymode)?.keymode;
        let md5 = parse_md5(&req.md5)?;
        let seed = parse_seed(&req.seed)?;
        let window = self.window_len(req.window_ms)?;
        let exclude = self.exclude(keymode, &req.exclude).await?;
        let catalog = self.catalog(keymode).await?;
        self.window_in(&catalog, keymode, md5, seed, window, &exclude)
            .await?
            .ok_or_else(|| AppError::conflict().with_arg("md5", req.md5.clone()))
    }

    /// A free window of any eligible chart, deterministic for seed, round and `exclude`; `None`
    /// once no chart has one left.
    pub async fn random(&self, req: RandomRequestDto) -> Result<Option<LabelWindowDto>, AppError> {
        let keymode = labelled_profile(req.keymode)?.keymode;
        let seed = parse_seed(&req.seed)?;
        let window = self.window_len(req.window_ms)?;
        let exclude = self.exclude(keymode, &req.exclude).await?;
        let catalog = self.catalog(keymode).await?;
        let order = sampler::random_order(seed, req.round, catalog.facts.iter().map(|c| c.md5));
        for md5 in order {
            let Some(rows) = rows_or_none(self.ctx, md5).await? else {
                continue;
            };
            let starts = sampler::window_starts(md5, &rows, window, &exclude, self.params.min_rows);
            if let Some(t0) = sampler::random_start(seed, req.round, md5, &starts) {
                return catalog
                    .window(md5, t0, TimeUs(t0.0 + window.0), keymode)
                    .map(Some);
            }
        }
        Ok(None)
    }

    /// A free window of the chart osu! stable is playing (its window title), else of the newest
    /// self replay in `Data/r`, else `None`; `req.exclude` keeps it off windows the session
    /// already showed. Probes once per call: shells call it on a button press, never on a timer
    /// (D9).
    pub async fn now_playing(
        &self,
        req: NowPlayingRequestDto,
    ) -> Result<Option<NowPlayingDto>, AppError> {
        let keymode = labelled_profile(req.keymode)?.keymode;
        let catalog = self.catalog(keymode).await?;
        let probe = self.probe.clone();
        let title = tokio::task::spawn_blocking(move || probe.window_title())
            .await
            .map_err(blocking_join_error)?;
        let names = self.ctx.players().self_alias_names().await?;
        let root = self.replay_root().await?;
        let newest_self = |among: BTreeSet<ChartMd5>| {
            newest_self_replay(
                root.clone(),
                names.clone(),
                among,
                self.now_playing.max_replay_headers,
            )
        };

        let named: Vec<ChartMd5> = title
            .as_deref()
            .and_then(parse_osu_title)
            .map(|t| title_matches(&t.artist_title_version, &catalog.charts))
            .unwrap_or_default()
            .into_iter()
            .filter_map(|c| c.md5.parse().ok())
            .collect();
        let from_title = match named.as_slice() {
            [] => None,
            [one] => Some(*one),
            // The same metadata on several charts: the one the user played last, else none.
            many => newest_self(many.iter().copied().collect()).await?,
        };
        let exclude = self.exclude(keymode, &req.exclude).await?;
        let free_window = |md5| {
            self.window_in(
                &catalog,
                keymode,
                md5,
                NOW_PLAYING_SEED,
                self.params.window,
                &exclude,
            )
        };
        if let Some(md5) = from_title
            && let Some(window) = free_window(md5).await?
        {
            return Ok(Some(NowPlayingDto {
                window,
                source: NowPlayingSourceDto::OsuWindow,
            }));
        }
        // A title chart with no free window left cannot stand in for itself as the last replay.
        let fallback = catalog
            .facts
            .iter()
            .map(|c| c.md5)
            .filter(|md5| Some(*md5) != from_title)
            .collect();
        let Some(md5) = newest_self(fallback).await? else {
            return Ok(None);
        };
        Ok(free_window(md5).await?.map(|window| NowPlayingDto {
            window,
            source: NowPlayingSourceDto::LastReplay,
        }))
    }

    /// Short keys or full ids of the keymode's taxonomy, case-insensitive, in input order
    /// without repeats.
    pub fn resolve_patterns(
        &self,
        keymode: u8,
        tokens: &[String],
    ) -> Result<Vec<PatternId>, AppError> {
        let profile = labelled_profile(keymode)?;
        if tokens.is_empty() {
            return Err(AppError::invalid_input().with_arg("patterns", ""));
        }
        let mut out: Vec<PatternId> = Vec::new();
        for token in tokens {
            let def =
                taxonomy::resolve(profile.taxonomy, &token.to_lowercase()).ok_or_else(|| {
                    AppError::invalid_input()
                        .with_key(keys::UNKNOWN_PATTERN)
                        .with_arg("pattern", token.clone())
                })?;
            if !out.contains(&def.id) {
                out.push(def.id.clone());
            }
        }
        Ok(out)
    }

    /// `anchor` widened, narrowed or shifted, kept inside the chart's rows.
    pub async fn reshape(&self, anchor: AnchorDto, op: WindowOpDto) -> Result<AnchorDto, AppError> {
        let md5 = parse_md5(&anchor.md5)?;
        let rows = self.ctx.library().row_times(md5).await?;
        let a = parse_anchor(md5, &anchor, rows.keymode)?;
        let span = window::chart_span(&rows.times)
            .ok_or_else(|| AppError::not_found().with_arg("md5", anchor.md5.clone()))?;
        let how = match op {
            WindowOpDto::Widen => Reshape::Widen,
            WindowOpDto::Narrow => Reshape::Narrow,
            WindowOpDto::Next => Reshape::Next,
            WindowOpDto::Prev => Reshape::Prev,
        };
        let p = &self.params;
        let (t0, t1) = window::reshape(
            a.t0_us(),
            a.t1_us(),
            how,
            span,
            p.reshape_step,
            p.min_window,
        )
        .map_err(|_| {
            AppError::invalid_input()
                .with_key(keys::WINDOW_TOO_SHORT)
                .with_arg("minMs", p.min_window.as_ms_floor().to_string())
        })?;
        let reshaped = SegmentAnchor::new(md5, t0, t1, a.cols(), rows.keymode)
            .map_err(|e| AppError::internal(format!("reshaped anchor: {e}")))?;
        anchor_dto(&reshaped)
    }

    /// `req.anchor` moved to start at `req.t0Ms` with its length, slid back inside the chart's
    /// rows. Columns are recomputed as the sampler sets them (every column), whatever the
    /// request carried.
    pub async fn move_window(&self, req: MoveWindowRequestDto) -> Result<AnchorDto, AppError> {
        let md5 = parse_md5(&req.anchor.md5)?;
        let rows = self.ctx.library().row_times(md5).await?;
        let a = parse_anchor(md5, &req.anchor, rows.keymode)?;
        let span = window::chart_span(&rows.times)
            .ok_or_else(|| AppError::not_found().with_arg("md5", req.anchor.md5.clone()))?;
        let len = TimeUs(a.t1_us().0 - a.t0_us().0);
        let (t0, t1) = window::move_to(TimeUs::from_ms(req.t0_ms), len, span);
        let moved = SegmentAnchor::new(md5, t0, t1, ColMask::full(rows.keymode), rows.keymode)
            .map_err(|e| AppError::internal(format!("moved anchor: {e}")))?;
        anchor_dto(&moved)
    }

    /// The chart's span cut into `req.buckets` equal slices with their note counts, plus the
    /// self profile's labelled windows on it. `NOT_FOUND` for a chart of another keymode or
    /// without a parse.
    pub async fn chart_timeline(
        &self,
        req: ChartTimelineRequestDto,
    ) -> Result<ChartTimelineDto, AppError> {
        let keymode = labelled_profile(req.keymode)?.keymode;
        if req.buckets == 0 || req.buckets > self.params.max_timeline_buckets {
            return Err(AppError::invalid_input().with_arg("buckets", req.buckets.to_string()));
        }
        let md5 = parse_md5(&req.md5)?;
        let not_found = || AppError::not_found().with_arg("md5", req.md5.clone());
        let rows = self.ctx.library().row_times(md5).await?;
        if rows.keymode != keymode {
            return Err(not_found());
        }
        let (first, end) = window::chart_span(&rows.times).ok_or_else(not_found)?;
        let mut density = vec![0_u16; usize::from(req.buckets)];
        let width = i128::from(end.0 - first.0);
        for (t, notes) in rows.times.iter().zip(&rows.notes) {
            let at = i128::from(t.0 - first.0) * i128::from(req.buckets) / width;
            let slot = usize::try_from(at).unwrap_or(0).min(density.len() - 1);
            density[slot] = density[slot].saturating_add(u16::from(*notes));
        }
        let mut labelled: Vec<SpanDto> = match self.self_profile().await? {
            Some(me) => self
                .labels(me)
                .await?
                .into_iter()
                .filter(|l| l.anchor.chart_md5() == md5)
                .map(|l| {
                    anchor_dto(&l.anchor).map(|a| SpanDto {
                        t0_ms: a.t0_ms,
                        t1_ms: a.t1_ms,
                    })
                })
                .collect::<Result<_, _>>()?,
            None => Vec::new(),
        };
        labelled.sort_by_key(|s| (s.t0_ms, s.t1_ms));
        Ok(ChartTimelineDto {
            first_ms: ms_i32(first)?,
            end_ms: ms_i32(end)?,
            density,
            labelled,
        })
    }

    /// Appends one `segment_label` event under the self profile. A skip is never stored.
    pub async fn submit(&self, req: LabelSubmitDto) -> Result<LabelEventDto, AppError> {
        let md5 = parse_md5(&req.anchor.md5)?;
        let me = self.require_self().await?;
        let rows = self.ctx.library().row_times(md5).await?;
        let profile = labelled_profile(rows.keymode.columns())?;
        let anchor = parse_anchor(md5, &req.anchor, rows.keymode)?;
        let inside = window::chart_span(&rows.times)
            .is_some_and(|span| window::within(anchor.t0_us(), anchor.t1_us(), span));
        if !inside {
            return Err(AppError::invalid_input()
                .with_arg("t0Ms", req.anchor.t0_ms.to_string())
                .with_arg("t1Ms", req.anchor.t1_ms.to_string()));
        }
        if req.no_pattern != req.patterns.is_empty() {
            return Err(AppError::invalid_input()
                .with_arg("patterns", req.patterns.join(","))
                .with_arg("noPattern", req.no_pattern.to_string()));
        }
        let answer = if req.no_pattern {
            GoldAnswer::NoPattern
        } else {
            GoldAnswer::Patterns(
                req.patterns
                    .iter()
                    .map(|p| {
                        taxonomy::by_id(profile.taxonomy, p)
                            .map(|def| def.id.clone())
                            .ok_or_else(|| AppError::invalid_input().with_arg("pattern", p.clone()))
                    })
                    .collect::<Result<Vec<PatternId>, _>>()?,
            )
        };
        let thumb_pref = req.thumb_pref.map(|t| match t {
            ThumbPrefDto::Left => ThumbPref::Left,
            ThumbPrefDto::Right => ThumbPref::Right,
        });
        let now = self.ctx.clock().now();
        let label = move |id| NewGoldLabel {
            id,
            ts: now,
            profile_id: me,
            keymode: rows.keymode,
            anchor,
            answer,
            mixed: req.mixed,
            unsure: req.unsure,
            thumb_pref,
            app_version: APP_VERSION.to_owned(),
        };
        let id = self
            .append(now, move |tx, id| {
                // Checked in the writing transaction, so two submits cannot both pass it.
                let clash = gold_labels(tx.conn(), me)?
                    .into_iter()
                    .find(|l| l.anchor.overlaps(&anchor));
                if let Some(l) = clash {
                    return Err(StoreError::Conflict(format!(
                        "window overlaps label {}",
                        l.id
                    )));
                }
                append_gold_label(tx, &label(id))
            })
            .await?;
        Ok(LabelEventDto { id: id.to_string() })
    }

    /// Appends an `undo` event that cancels one of the self profile's labels.
    pub async fn undo(&self, event_id: &str) -> Result<(), AppError> {
        let target = ulid::Ulid::from_string(event_id)
            .map_err(|_| AppError::invalid_input().with_arg("eventId", event_id))?;
        let me = self.require_self().await?;
        let now = self.ctx.clock().now();
        self.append(now, move |tx, id| {
            append_undo(
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

    pub async fn stats(&self) -> Result<LabelStatsDto, AppError> {
        let labels = match self.self_profile().await? {
            Some(me) => self.labels(me).await?,
            None => Vec::new(),
        };
        let mut per_pattern = BTreeMap::new();
        let mut per_axis = BTreeMap::new();
        let mut per_stratum = BTreeMap::new();
        let keymodes: BTreeSet<Keymode> = labels.iter().map(|l| l.keymode).collect();
        let mut strata: BTreeMap<ChartMd5, Assignment> = BTreeMap::new();
        for keymode in keymodes {
            let charts = self.ctx.library().overview(keymode).await?;
            let played = self.played().await?;
            strata.extend(sampler::assign(
                &chart_facts(&charts, &played),
                &self.params,
            ));
        }
        let registry = Registry::builtin();
        for l in &labels {
            let defs = registry.profile(l.keymode).map_or(&[][..], |p| p.taxonomy);
            let patterns = patterns_of(l);
            let axes: BTreeSet<String> = patterns
                .iter()
                .filter_map(|p| taxonomy::by_id(defs, p.as_str()))
                .map(|d| d.axis.to_string())
                .collect();
            for p in patterns {
                *per_pattern.entry(p.to_string()).or_insert(0) += 1;
            }
            for axis in axes {
                *per_axis.entry(axis).or_insert(0) += 1;
            }
            let stratum = strata
                .get(&l.anchor.chart_md5())
                .map_or_else(|| UNKNOWN_STRATUM.to_owned(), |a| a.stratum.to_string());
            *per_stratum.entry(stratum).or_insert(0) += 1;
        }
        Ok(LabelStatsDto {
            total: count(labels.len()),
            no_pattern: count(
                labels
                    .iter()
                    .filter(|l| l.answer == GoldAnswer::NoPattern)
                    .count(),
            ),
            mixed: count(labels.iter().filter(|l| l.mixed).count()),
            unsure: count(labels.iter().filter(|l| l.unsure).count()),
            thumb_left: count(
                labels
                    .iter()
                    .filter(|l| l.thumb_pref == Some(ThumbPref::Left))
                    .count(),
            ),
            thumb_right: count(
                labels
                    .iter()
                    .filter(|l| l.thumb_pref == Some(ThumbPref::Right))
                    .count(),
            ),
            per_pattern: counts(per_pattern),
            per_axis: counts(per_axis),
            per_stratum: counts(per_stratum),
        })
    }

    /// The gold set as JSONL, sorted by (md5, t0).
    pub async fn export_jsonl(&self) -> Result<String, AppError> {
        let labels = match self.self_profile().await? {
            Some(me) => self.labels(me).await?,
            None => Vec::new(),
        };
        export_lines(labels)
    }

    /// Writes [`Self::export_jsonl`] to `path`, creating its directory. Never into an osu!
    /// install: only `app::export` may write there (D9).
    pub async fn export_to(&self, path: PathBuf) -> Result<LabelExportDto, AppError> {
        let text = self.export_jsonl().await?;
        let roots: Vec<PathBuf> = self
            .ctx
            .installs()
            .await?
            .into_iter()
            .map(|i| i.root_path)
            .collect();
        tokio::task::spawn_blocking(move || {
            let cwd = std::env::current_dir()
                .map_err(|e| AppError::internal(format!("working directory: {e}")))?;
            let target = export_target(&path, &cwd, &roots)?;
            if let Some(dir) = target.parent() {
                std::fs::create_dir_all(dir)
                    .map_err(|e| AppError::internal(format!("create {}: {e}", dir.display())))?;
                // Re-checked on the real directory: a link created meanwhile cannot redirect
                // the write into an install.
                let real = std::fs::canonicalize(dir)
                    .map_err(|e| AppError::internal(format!("resolve {}: {e}", dir.display())))?;
                if roots.iter().any(|root| inside(&real, root)) {
                    return Err(outside_installs_error(&path));
                }
            }
            std::fs::write(&target, &text)
                .map_err(|e| AppError::internal(format!("write {}: {e}", target.display())))?;
            Ok(LabelExportDto {
                path: path.to_string_lossy().into_owned(),
                rows: count(text.lines().count()),
            })
        })
        .await
        .map_err(blocking_join_error)?
    }

    fn window_len(&self, window_ms: Option<u32>) -> Result<TimeUs, AppError> {
        match window_ms {
            None => Ok(self.params.window),
            Some(0) => Err(AppError::invalid_input().with_arg("windowMs", "0")),
            Some(ms) => Ok(TimeUs(i64::from(ms) * US_PER_MS)),
        }
    }

    /// `shown` plus the self profile's stored labels, which are always avoided.
    async fn exclude(
        &self,
        keymode: Keymode,
        shown: &[AnchorDto],
    ) -> Result<Vec<SegmentAnchor>, AppError> {
        let mut out = shown
            .iter()
            .map(|a| parse_md5(&a.md5).and_then(|md5| parse_anchor(md5, a, keymode)))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(me) = self.self_profile().await? {
            out.extend(self.labels(me).await?.into_iter().map(|l| l.anchor));
        }
        Ok(out)
    }

    async fn catalog(&self, keymode: Keymode) -> Result<Catalog, AppError> {
        let charts = self.ctx.library().overview(keymode).await?;
        let played = self.played().await?;
        let facts = chart_facts(&charts, &played);
        let assigned = sampler::assign(&facts, &self.params);
        Ok(Catalog {
            charts,
            played,
            facts,
            assigned,
        })
    }

    /// A free window of `md5` picked as the sampler picks starts (round 0); `None` when every
    /// window overlaps `exclude`.
    async fn window_in(
        &self,
        catalog: &Catalog,
        keymode: Keymode,
        md5: ChartMd5,
        seed: u64,
        window: TimeUs,
        exclude: &[SegmentAnchor],
    ) -> Result<Option<LabelWindowDto>, AppError> {
        let not_found = || AppError::not_found().with_arg("md5", md5.to_string());
        if !catalog.facts.iter().any(|c| c.md5 == md5) {
            return Err(not_found());
        }
        let rows = rows_or_none(self.ctx, md5).await?.ok_or_else(not_found)?;
        let starts = sampler::window_starts(md5, &rows, window, exclude, self.params.min_rows);
        sampler::pick_start(seed, 0, md5, &starts)
            .map(|t0| catalog.window(md5, t0, TimeUs(t0.0 + window.0), keymode))
            .transpose()
    }

    /// The install whose osu!.db built the catalog: its `Data/r` holds the replays of the
    /// charts listed. `None` without a catalog.
    async fn replay_root(&self) -> Result<Option<PathBuf>, AppError> {
        let (user, cache) = (self.ctx.user_db().clone(), self.ctx.cache_db().clone());
        match tokio::task::spawn_blocking(move || catalog_install(&user, &cache))
            .await
            .map_err(blocking_join_error)?
        {
            Ok(install) => Ok(install.map(|i| i.root_path)),
            Err(e) if e.code == ErrorCode::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn self_profile(&self) -> Result<Option<ProfileId>, AppError> {
        self.ctx.players().self_profile_id().await
    }

    /// Charts the user played: other players' plays never mark a chart (skill scope rule).
    async fn played(&self) -> Result<BTreeSet<ChartMd5>, AppError> {
        let aliases = self.ctx.players().self_alias_ids().await?;
        self.ctx.plays().played_charts(&aliases).await
    }

    /// Labels belong to the user; the self profile exists once identity has been refreshed
    /// (every sync chains that).
    async fn require_self(&self) -> Result<ProfileId, AppError> {
        self.self_profile()
            .await?
            .ok_or_else(|| AppError::not_found().with_arg("profile", "self"))
    }

    async fn labels(&self, me: ProfileId) -> Result<Vec<GoldLabel>, AppError> {
        let user = self.ctx.user_db().clone();
        let labels = tokio::task::spawn_blocking(move || user.read(|c| gold_labels(c, me)))
            .await
            .map_err(blocking_join_error)??;
        Ok(labels)
    }

    /// Runs `write` with the next event id inside one transaction, so ids stay strictly
    /// increasing even when events share a millisecond (undo takes "last" by id order).
    async fn append(
        &self,
        now: UnixUs,
        write: impl FnOnce(&wolluf_store::Tx<'_>, ulid::Ulid) -> Result<(), wolluf_store::StoreError>
        + Send
        + 'static,
    ) -> Result<ulid::Ulid, AppError> {
        let user = self.ctx.user_db().clone();
        let id = tokio::task::spawn_blocking(move || {
            user.write(move |tx| {
                let id = next_id(now, feedback_event::last_id(tx.conn())?);
                write(tx, id)?;
                Ok(id)
            })
        })
        .await
        .map_err(blocking_join_error)??;
        Ok(id)
    }
}

fn labelled_profile(keymode: u8) -> Result<&'static KeymodeProfile, AppError> {
    Keymode::new(keymode)
        .ok()
        .and_then(|k| Registry::builtin().profile(k))
        .filter(|p| !p.taxonomy.is_empty())
        .ok_or_else(|| AppError::invalid_input().with_arg("keymode", keymode.to_string()))
}

fn parse_seed(seed: &str) -> Result<u64, AppError> {
    seed.parse()
        .map_err(|_| AppError::invalid_input().with_arg("seed", seed))
}

/// A chart's row times; `None` once it was re-indexed away after the overview was read.
async fn rows_or_none(ctx: &AppContext, md5: ChartMd5) -> Result<Option<Vec<TimeUs>>, AppError> {
    match ctx.library().row_times(md5).await {
        Ok(rows) => Ok(Some(rows.times)),
        Err(e) if e.code == ErrorCode::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// The chart of the newest `.osr` in `<root>/Data/r` among `among` whose player is one of
/// `names` (ADR 0005: another player's replay never stands for the user). Read live, so a
/// replay saved since the last sync counts; at most `max_headers` headers are read.
async fn newest_self_replay(
    root: Option<PathBuf>,
    names: Vec<Vec<u8>>,
    among: BTreeSet<ChartMd5>,
    max_headers: usize,
) -> Result<Option<ChartMd5>, AppError> {
    let Some(root) = root.filter(|_| !names.is_empty()) else {
        return Ok(None);
    };
    tokio::task::spawn_blocking(move || {
        let mut replays: Vec<(FileTime, ChartMd5, PathBuf)> = replay_dir::index(&root)?
            .into_iter()
            .filter(|((md5, _), _)| among.contains(md5))
            .filter_map(|((md5, filetime), files)| Some((filetime, md5, files.osr?)))
            .collect();
        replays.sort_unstable_by(|a, b| b.cmp(a));
        for (_, md5, path) in replays.into_iter().take(max_headers) {
            match replay_player(&path) {
                Ok(player) if names.contains(&player) => return Ok(Some(md5)),
                Ok(_) => {}
                // osu! may still be writing it; an older replay answers instead.
                Err(e) => tracing::debug!(error = %e, path = %path.display(), "replay header"),
            }
        }
        Ok(None)
    })
    .await
    .map_err(blocking_join_error)?
}

fn next_id(now: UnixUs, last: Option<ulid::Ulid>) -> ulid::Ulid {
    let fresh = ulid::Ulid::from_datetime(to_system_time(now));
    match last {
        // 80 random bits cannot run out in practice; on overflow the insert fails on the key.
        Some(last) if fresh <= last => last.increment().unwrap_or(last),
        _ => fresh,
    }
}

fn parse_md5(md5: &str) -> Result<ChartMd5, AppError> {
    md5.parse()
        .map_err(|_| AppError::invalid_input().with_arg("md5", md5))
}

fn parse_anchor(md5: ChartMd5, a: &AnchorDto, keymode: Keymode) -> Result<SegmentAnchor, AppError> {
    let invalid = || {
        AppError::invalid_input()
            .with_arg("t0Ms", a.t0_ms.to_string())
            .with_arg("t1Ms", a.t1_ms.to_string())
            .with_arg("cols", format!("{:?}", a.cols))
    };
    let zero_based = a
        .cols
        .iter()
        .map(|&c| c.checked_sub(1).ok_or_else(invalid))
        .collect::<Result<Vec<u8>, _>>()?;
    let cols = ColMask::from_cols(keymode, zero_based).map_err(|_| invalid())?;
    SegmentAnchor::new(
        md5,
        TimeUs::from_ms(a.t0_ms),
        TimeUs::from_ms(a.t1_ms),
        cols,
        keymode,
    )
    .map_err(|_| invalid())
}

fn ms_i32(t: TimeUs) -> Result<i32, AppError> {
    i32::try_from(t.as_ms_floor())
        .map_err(|_| AppError::internal(format!("chart time {} out of range", t.0)))
}

fn anchor_dto(a: &SegmentAnchor) -> Result<AnchorDto, AppError> {
    Ok(AnchorDto {
        md5: a.chart_md5().to_string(),
        t0_ms: ms_i32(a.t0_us())?,
        t1_ms: ms_i32(a.t1_us())?,
        cols: one_based(a.cols()),
    })
}

fn one_based(cols: ColMask) -> Vec<u8> {
    cols.iter().map(|c| c + 1).collect()
}

fn chart_facts(charts: &[LibraryChartDto], played: &BTreeSet<ChartMd5>) -> Vec<ChartFacts> {
    charts
        .iter()
        .filter_map(|c| {
            let md5: ChartMd5 = c.md5.parse().ok()?;
            Some(ChartFacts {
                md5,
                nps: c.nps,
                played: played.contains(&md5),
                labels: c
                    .labels
                    .iter()
                    // The gold protocol stays blind: name hints never reach the sampler.
                    .filter(|l| l.source != label_source::NAME_HINT)
                    .map(|l| LevelLabel {
                        source: l.source.clone(),
                        scale: l.scale.clone(),
                        level_ord: l.level_ord,
                        level_text: l.level_text.clone(),
                    })
                    .collect(),
            })
        })
        .collect()
}

fn export_lines(mut labels: Vec<GoldLabel>) -> Result<String, AppError> {
    labels.sort_by_key(|l| {
        let a = l.anchor;
        (a.chart_md5(), a.t0_us(), a.t1_us(), l.id)
    });
    let mut out = String::new();
    for l in labels {
        let mut flags = Vec::new();
        if l.mixed {
            flags.push(FLAG_MIXED);
        }
        if l.unsure {
            flags.push(FLAG_UNSURE);
        }
        let row = ExportRow {
            md5: l.anchor.chart_md5().to_string(),
            t0_us: l.anchor.t0_us().0,
            t1_us: l.anchor.t1_us().0,
            cols: one_based(l.anchor.cols()),
            patterns: patterns_of(&l).iter().map(ToString::to_string).collect(),
            no_pattern: l.answer == GoldAnswer::NoPattern,
            flags,
            thumb_pref: l.thumb_pref.map(ThumbPref::as_str),
            labelled_at: format_rfc3339_ms(l.ts),
        };
        let line = serde_json::to_string(&row)
            .map_err(|e| AppError::internal(format!("export row: {e}")))?;
        out.push_str(&line);
        out.push('\n');
    }
    Ok(out)
}

fn patterns_of(l: &GoldLabel) -> &[PatternId] {
    match &l.answer {
        GoldAnswer::Patterns(p) => p,
        GoldAnswer::NoPattern => &[],
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn counts(map: BTreeMap<String, usize>) -> Vec<CountDto> {
    map.into_iter()
        .map(|(key, n)| CountDto {
            key,
            count: count(n),
        })
        .collect()
}

fn outside_installs_error(path: &Path) -> AppError {
    AppError::invalid_input().with_arg("path", path.to_string_lossy())
}

/// The absolute file `path` names, resolved against `cwd`. `..` is refused outright rather
/// than resolved, and the result must lie outside every install root (D9).
fn export_target(path: &Path, cwd: &Path, roots: &[PathBuf]) -> Result<PathBuf, AppError> {
    if path.components().any(|c| c == Component::ParentDir) || path.file_name().is_none() {
        return Err(outside_installs_error(path));
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    if roots.iter().any(|root| inside(&absolute, root)) {
        return Err(outside_installs_error(path));
    }
    Ok(absolute)
}

/// Component-wise against the canonical root. The target may not exist yet, so its nearest
/// existing ancestor is canonicalized (resolving links) and the rest appended; `..` never
/// reaches here.
fn inside(path: &Path, root: &Path) -> bool {
    let canonical = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let mut existing = path;
    let mut rest = Vec::new();
    while !existing.exists() {
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_owned());
                existing = parent;
            }
            _ => break,
        }
    }
    let mut full = canonical(existing);
    full.extend(rest.iter().rev());
    full.starts_with(canonical(root))
}

#[cfg(test)]
mod tests;
