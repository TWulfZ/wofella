//! `wolluf players list` over `PlayersService::list_aliases` (spec 005, 004's alias table).
//!
//! 004 T11 has not added `AliasListDto` yet, so this module mirrors its documented wire shape
//! (spec 004 IPC) from the domain `AliasList`. Once the DTO lands, serialize it directly and
//! delete the mirror and the timestamp formatter.

use std::process::ExitCode;

use serde::Serialize;
use wolluf_app::context::AppContext;
use wolluf_app::features::players::identity::{AliasList, AliasRow};
use wolluf_core::UnixUs;

use crate::cli::PlayersCmd;
use crate::exit;
use crate::render;

/// Shown for the empty player name, which osu! stores for guest plays.
const EMPTY_NAME: &str = "(empty)";

pub(crate) async fn run(ctx: &AppContext, cmd: PlayersCmd, json: bool) -> anyhow::Result<ExitCode> {
    match cmd {
        PlayersCmd::List => {
            let list = ctx.players().list_aliases().await?;
            if json {
                render::json(&AliasListView::from(&list))?;
            } else {
                render::text(&aliases_table(&list))?;
            }
        }
    }
    Ok(exit::exit_code(exit::SUCCESS))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AliasListView {
    selection_version: u32,
    cfg_username_available: bool,
    wizard_needed: bool,
    aliases: Vec<AliasRowView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AliasRowView {
    alias_id: i64,
    raw_name: String,
    is_empty_name: bool,
    normalized_length: u32,
    n_plays: u32,
    by_keymode: Vec<KeymodeCountView>,
    first_played_at: Option<String>,
    last_played_at: Option<String>,
    n_online: u32,
    n_offline: u32,
    n_with_replay: u32,
    top_charts: Vec<TopChartView>,
    auto_match: Option<AutoMatchView>,
    decision: Option<&'static str>,
    selected: bool,
    in_self_profile: bool,
}

#[derive(Serialize)]
struct KeymodeCountView {
    bucket: String,
    n: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TopChartView {
    chart_md5: String,
    title: Option<String>,
    version: Option<String>,
    n: u32,
}

#[derive(Serialize)]
struct AutoMatchView {
    source: &'static str,
    kind: &'static str,
}

impl From<&AliasList> for AliasListView {
    fn from(list: &AliasList) -> Self {
        Self {
            selection_version: list.selection_version,
            cfg_username_available: list.cfg_username_available,
            wizard_needed: list.wizard_needed,
            aliases: list.rows.iter().map(AliasRowView::from).collect(),
        }
    }
}

impl From<&AliasRow> for AliasRowView {
    fn from(row: &AliasRow) -> Self {
        let s = &row.stats;
        Self {
            alias_id: row.alias_id.0,
            raw_name: String::from_utf8_lossy(&row.raw_name).into_owned(),
            is_empty_name: row.raw_name.is_empty(),
            normalized_length: row.norm_len,
            n_plays: s.n_plays,
            by_keymode: s
                .by_keymode
                .iter()
                .map(|(bucket, n)| KeymodeCountView {
                    bucket: bucket.to_string(),
                    n: *n,
                })
                .collect(),
            first_played_at: s.first_played_at.map(rfc3339_ms),
            last_played_at: s.last_played_at.map(rfc3339_ms),
            n_online: s.n_online,
            n_offline: s.n_plays.saturating_sub(s.n_online),
            n_with_replay: s.n_with_replay,
            top_charts: row
                .top_charts
                .iter()
                .map(|c| TopChartView {
                    chart_md5: c.chart_md5.to_string(),
                    title: c.title.clone(),
                    version: c.version.clone(),
                    n: c.n,
                })
                .collect(),
            auto_match: row.auto_match.map(|m| AutoMatchView {
                source: m.source.as_str(),
                kind: m.kind.as_str(),
            }),
            decision: row.decision.map(|d| d.as_str()),
            selected: row.selected,
            in_self_profile: row.in_self_profile,
        }
    }
}

/// Rows stay in the service's R6 order (auto matches first, then by play count).
fn aliases_table(list: &AliasList) -> String {
    let rows: Vec<Vec<String>> = list
        .rows
        .iter()
        .map(|row| {
            let view = AliasRowView::from(row);
            let keymodes: Vec<String> = view
                .by_keymode
                .iter()
                .map(|k| format!("{}:{}", k.bucket, k.n))
                .collect();
            vec![
                if view.selected { "x" } else { "" }.to_owned(),
                render::opt(view.decision),
                render::opt(view.auto_match.map(|m| format!("{}/{}", m.source, m.kind))),
                view.n_plays.to_string(),
                if keymodes.is_empty() {
                    "-".to_owned()
                } else {
                    keymodes.join(",")
                },
                render::opt(view.last_played_at),
                if view.is_empty_name {
                    EMPTY_NAME.to_owned()
                } else {
                    view.raw_name
                },
            ]
        })
        .collect();
    let mut out = render::table(
        &[
            "SEL", "DECISION", "AUTO", "PLAYS", "KEYMODES", "LAST", "NAME",
        ],
        &rows,
    );
    if list.wizard_needed {
        out.push_str("identity wizard pending: no alias is confirmed yet\n");
    }
    out
}

// Same shape as the app's persisted timestamps (RFC 3339 UTC, milliseconds, floored). Civil
// date from H. Hinnant's civil_from_days; years start in March so the leap day is last.
const US_PER_MS: i64 = 1_000;
const MS_PER_SEC: i64 = 1_000;
const SECS_PER_MIN: i64 = 60;
const SECS_PER_HOUR: i64 = 3_600;
const SECS_PER_DAY: i64 = 86_400;
const DAYS_PER_ERA: i64 = 146_097;
const YEARS_PER_ERA: i64 = 400;
/// Days from 0000-03-01 to 1970-01-01.
const UNIX_EPOCH_SHIFT: i64 = 719_468;

pub(crate) fn rfc3339_ms(t: UnixUs) -> String {
    let ms = t.0.div_euclid(US_PER_MS);
    let secs = ms.div_euclid(MS_PER_SEC);
    let days = secs.div_euclid(SECS_PER_DAY);
    let sod = secs.rem_euclid(SECS_PER_DAY);
    let z = days + UNIX_EPOCH_SHIFT;
    let era = z.div_euclid(DAYS_PER_ERA);
    let doe = z - era * DAYS_PER_ERA;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / (DAYS_PER_ERA - 1)) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * YEARS_PER_ERA + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        sod / SECS_PER_HOUR,
        sod % SECS_PER_HOUR / SECS_PER_MIN,
        sod % SECS_PER_MIN,
        ms.rem_euclid(MS_PER_SEC)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_matches_known_instants() {
        assert_eq!(
            rfc3339_ms(UnixUs(1_790_637_236_636_999)),
            "2026-09-28T23:13:56.636Z"
        );
        assert_eq!(
            rfc3339_ms(UnixUs(951_782_400_000_000)),
            "2000-02-29T00:00:00.000Z"
        );
        assert_eq!(rfc3339_ms(UnixUs(-1)), "1969-12-31T23:59:59.999Z");
    }
}
