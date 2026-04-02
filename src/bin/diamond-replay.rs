use std::collections::HashMap;
use std::io::{self, stdout};
use std::{env, fs, process};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
};
use ratatui::Terminal;

use diamond_replay::player::{BattingStats, PitchingStats, PlayerGameStats};
use diamond_replay::replay::{GameResult, LittleLeagueStats};
use diamond_replay::replay_from_json;

// ---------------------------------------------------------------------------
// Style constants
// ---------------------------------------------------------------------------

const TITLE_STYLE: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
const HEADER_STYLE: Style = Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD);
const DIM_STYLE: Style = Style::new().fg(Color::DarkGray);

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    BoxScore,
    Batting,
    Pitching,
    LittleLeague,
}

impl View {
    const fn label(self) -> &'static str {
        match self {
            Self::BoxScore => "Box Score",
            Self::Batting => "Batting",
            Self::Pitching => "Pitching",
            Self::LittleLeague => "Little League",
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::BoxScore => Self::Batting,
            Self::Batting => Self::Pitching,
            Self::Pitching => Self::LittleLeague,
            Self::LittleLeague => Self::BoxScore,
        }
    }
}

struct App {
    game_name: String,
    view: View,
    scroll: u16,
    viewport_height: u16,
    boxscore_content: Vec<Line<'static>>,
    batting_content: Vec<Line<'static>>,
    pitching_content: Vec<Line<'static>>,
    ll_content: Vec<Line<'static>>,
}

impl App {
    fn new(result: &GameResult, game_name: String) -> Self {
        let away_team = truncate_team(&result.away_id, 20);
        let home_team = truncate_team(&result.home_id, 20);
        Self {
            game_name,
            view: View::BoxScore,
            scroll: 0,
            viewport_height: 0,
            boxscore_content: build_boxscore_lines(result, away_team, home_team),
            batting_content: build_adv_batting_lines(result, away_team, home_team),
            pitching_content: build_pitching_lines(result, away_team, home_team),
            ll_content: build_ll_lines(result, away_team, home_team),
        }
    }

    fn content_height(&self) -> u16 {
        let len = match self.view {
            View::BoxScore => self.boxscore_content.len(),
            View::Batting => self.batting_content.len(),
            View::Pitching => self.pitching_content.len(),
            View::LittleLeague => self.ll_content.len(),
        };
        u16::try_from(len).unwrap_or(u16::MAX)
    }

    fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    fn scroll_down(&mut self) {
        let max = self.content_height().saturating_sub(self.viewport_height);
        if self.scroll < max {
            self.scroll += 1;
        }
    }

    fn set_view(&mut self, view: View) {
        if self.view != view {
            self.view = view;
            self.scroll = 0;
        }
    }
}

fn truncate_team(name: &str, max: usize) -> &str {
    match name.char_indices().nth(max) {
        Some((idx, _)) => &name[..idx],
        None => name,
    }
}

// ---------------------------------------------------------------------------
// Helpers -- collect players by team, filtering anonymous
// ---------------------------------------------------------------------------

fn team_batters<'a>(
    stats: &'a HashMap<String, PlayerGameStats>,
    team_id: &str,
) -> Vec<&'a PlayerGameStats> {
    let mut players: Vec<&PlayerGameStats> = stats
        .values()
        .filter(|p| p.team_id == team_id && !p.player_id.starts_with("__anon_") && p.batting.pa > 0)
        .collect();
    players.sort_by(|a, b| b.batting.pa.cmp(&a.batting.pa));
    players
}

fn team_pitchers<'a>(
    stats: &'a HashMap<String, PlayerGameStats>,
    team_id: &str,
) -> Vec<&'a PlayerGameStats> {
    let mut players: Vec<&PlayerGameStats> = stats
        .values()
        .filter(|p| {
            p.team_id == team_id
                && !p.player_id.starts_with("__anon_")
                && p.pitching.is_some()
                && p.pitching
                    .as_ref()
                    .is_some_and(|ps| ps.outs_recorded > 0 || ps.bf > 0)
        })
        .collect();
    players.sort_by(|a, b| {
        let a_outs = a.pitching.as_ref().map_or(0, |p| p.outs_recorded);
        let b_outs = b.pitching.as_ref().map_or(0, |p| p.outs_recorded);
        b_outs.cmp(&a_outs)
    });
    players
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

/// Format a rate stat as ".XXX" (3 decimal places, leading dot).
fn fmt_avg(val: Option<f64>) -> String {
    match val {
        Some(v) => {
            if v >= 1.0 {
                format!("{v:.3}")
            } else {
                // Strip the leading zero: "0.333" -> ".333"
                let s = format!("{v:.3}");
                s.strip_prefix('0').unwrap_or(&s).to_string()
            }
        }
        None => "-".to_string(),
    }
}

/// Format a percentage as "XX.X".
fn fmt_pct(val: Option<f64>) -> String {
    match val {
        Some(v) => format!("{:.1}", v * 100.0),
        None => "-".to_string(),
    }
}

/// Format ERA/FIP/WHIP as "X.XX".
fn fmt_rate(val: Option<f64>) -> String {
    match val {
        Some(v) => format!("{v:.2}"),
        None => "-".to_string(),
    }
}

/// Format a per-9 or per-PA stat as "X.X".
fn fmt_per(val: Option<f64>) -> String {
    match val {
        Some(v) => format!("{v:.1}"),
        None => "-".to_string(),
    }
}

/// Style for integer values: dim if zero, default otherwise.
fn int_style(val: i32) -> Style {
    if val == 0 {
        DIM_STYLE
    } else {
        Style::default()
    }
}

/// Style for rate/avg values: dim if `None` or zero.
fn rate_style(val: Option<f64>) -> Style {
    match val {
        None | Some(0.0) => DIM_STYLE,
        _ => Style::default(),
    }
}

// ---------------------------------------------------------------------------
// Table row building
// ---------------------------------------------------------------------------

/// Build a fixed-width cell, right-aligned within the given width.
fn cell(text: &str, width: usize, style: Style) -> Span<'static> {
    let padded = format!("{text:>width$}");
    Span::styled(padded, style)
}

/// Build a fixed-width cell, left-aligned within the given width.
fn cell_left(text: &str, width: usize, style: Style) -> Span<'static> {
    let padded = format!("{text:<width$}");
    Span::styled(padded, style)
}

// ---------------------------------------------------------------------------
// Box Score view: linescore + basic batting tables
// ---------------------------------------------------------------------------

fn build_linescore_lines(
    result: &GameResult,
    away_team: &str,
    home_team: &str,
) -> Vec<Line<'static>> {
    let away = &result.linescore_away;
    let home = &result.linescore_home;
    let num_innings = away.len().max(home.len());

    let bold = Style::new().add_modifier(Modifier::BOLD);

    // Inning headers
    let mut header_spans = vec![cell_left("", 8, DIM_STYLE)];
    for i in 1..=num_innings {
        header_spans.push(cell(&i.to_string(), 4, DIM_STYLE));
    }
    header_spans.push(Span::styled("  \u{2502}  ", DIM_STYLE));
    header_spans.push(cell("R", 3, DIM_STYLE));

    // Away row
    let away_total: i32 = away.iter().sum();
    let mut away_spans = vec![cell_left(away_team, 8, bold)];
    for i in 0..num_innings {
        let val = away.get(i).copied().unwrap_or(0);
        away_spans.push(cell(&val.to_string(), 4, int_style(val)));
    }
    away_spans.push(Span::styled("  \u{2502}  ", DIM_STYLE));
    away_spans.push(cell(
        &away_total.to_string(),
        3,
        Style::new().add_modifier(Modifier::BOLD),
    ));

    // Home row
    let home_total: i32 = home.iter().sum();
    let mut home_spans = vec![cell_left(home_team, 8, bold)];
    for i in 0..num_innings {
        if i < home.len() {
            let val = home[i];
            home_spans.push(cell(&val.to_string(), 4, int_style(val)));
        } else {
            // Home didn't bat this inning
            home_spans.push(cell("x", 4, DIM_STYLE));
        }
    }
    home_spans.push(Span::styled("  \u{2502}  ", DIM_STYLE));
    home_spans.push(cell(
        &home_total.to_string(),
        3,
        Style::new().add_modifier(Modifier::BOLD),
    ));

    vec![
        Line::from(header_spans),
        Line::from(away_spans),
        Line::from(home_spans),
    ]
}

struct BattingTableColumns {
    name_w: usize,
    num_w: usize,
}

impl Default for BattingTableColumns {
    fn default() -> Self {
        Self {
            name_w: 20,
            num_w: 6,
        }
    }
}

fn build_batting_header(cols: &BattingTableColumns) -> Line<'static> {
    Line::from(vec![
        cell_left("Player", cols.name_w, HEADER_STYLE),
        cell("AB", cols.num_w, HEADER_STYLE),
        cell("H", cols.num_w, HEADER_STYLE),
        cell("AVG", cols.num_w, HEADER_STYLE),
        cell("OBP", cols.num_w, HEADER_STYLE),
        cell("SLG", cols.num_w, HEADER_STYLE),
        cell("OPS", cols.num_w, HEADER_STYLE),
        cell("R", cols.num_w, HEADER_STYLE),
        cell("RBI", cols.num_w, HEADER_STYLE),
        cell("BB", cols.num_w, HEADER_STYLE),
        cell("K", cols.num_w, HEADER_STYLE),
        cell("SB", cols.num_w, HEADER_STYLE),
    ])
}

fn build_batting_row(p: &PlayerGameStats, cols: &BattingTableColumns) -> Line<'static> {
    let b = &p.batting;
    let avg_s = fmt_avg(b.avg);
    let obp_s = fmt_avg(b.obp);
    let slg_s = fmt_avg(b.slg);
    let ops_s = fmt_avg(b.ops);
    Line::from(vec![
        cell_left(&p.player_id, cols.name_w, Style::default()),
        cell(&b.ab.to_string(), cols.num_w, int_style(b.ab)),
        cell(&b.hits.to_string(), cols.num_w, int_style(b.hits)),
        cell(&avg_s, cols.num_w, rate_style(b.avg)),
        cell(&obp_s, cols.num_w, rate_style(b.obp)),
        cell(&slg_s, cols.num_w, rate_style(b.slg)),
        cell(&ops_s, cols.num_w, rate_style(b.ops)),
        cell(&b.runs.to_string(), cols.num_w, int_style(b.runs)),
        cell(&b.rbi.to_string(), cols.num_w, int_style(b.rbi)),
        cell(&b.bb.to_string(), cols.num_w, int_style(b.bb)),
        cell(&b.k.to_string(), cols.num_w, int_style(b.k)),
        cell(&b.sb.to_string(), cols.num_w, int_style(b.sb)),
    ])
}

fn build_batting_total(stats: &BattingStats, cols: &BattingTableColumns) -> Line<'static> {
    let bold = Style::new().add_modifier(Modifier::BOLD);
    let avg_s = fmt_avg(stats.avg);
    let obp_s = fmt_avg(stats.obp);
    let slg_s = fmt_avg(stats.slg);
    let ops_s = fmt_avg(stats.ops);
    Line::from(vec![
        cell_left("TOTAL", cols.name_w, bold),
        cell(&stats.ab.to_string(), cols.num_w, bold),
        cell(&stats.hits.to_string(), cols.num_w, bold),
        cell(&avg_s, cols.num_w, bold),
        cell(&obp_s, cols.num_w, bold),
        cell(&slg_s, cols.num_w, bold),
        cell(&ops_s, cols.num_w, bold),
        cell(&stats.runs.to_string(), cols.num_w, bold),
        cell(&stats.rbi.to_string(), cols.num_w, bold),
        cell(&stats.bb.to_string(), cols.num_w, bold),
        cell(&stats.k.to_string(), cols.num_w, bold),
        cell(&stats.sb.to_string(), cols.num_w, bold),
    ])
}

fn build_boxscore_lines(
    result: &GameResult,
    away_team: &str,
    home_team: &str,
) -> Vec<Line<'static>> {
    let cols = BattingTableColumns::default();
    let mut lines = Vec::new();

    // Linescore
    lines.extend(build_linescore_lines(result, away_team, home_team));
    lines.push(Line::from(""));

    // Away batting
    lines.push(Line::from(Span::styled(
        format!(" BATTING \u{2500} {away_team} "),
        TITLE_STYLE,
    )));
    lines.push(build_batting_header(&cols));
    for p in team_batters(&result.player_stats, &result.away_id) {
        lines.push(build_batting_row(p, &cols));
    }
    lines.push(Line::from(Span::styled(
        "\u{2500}".repeat(cols.name_w + cols.num_w * 11),
        DIM_STYLE,
    )));
    lines.push(build_batting_total(&result.away_batting, &cols));
    lines.push(Line::from(""));

    // Home batting
    lines.push(Line::from(Span::styled(
        format!(" BATTING \u{2500} {home_team} "),
        TITLE_STYLE,
    )));
    lines.push(build_batting_header(&cols));
    for p in team_batters(&result.player_stats, &result.home_id) {
        lines.push(build_batting_row(p, &cols));
    }
    lines.push(Line::from(Span::styled(
        "\u{2500}".repeat(cols.name_w + cols.num_w * 11),
        DIM_STYLE,
    )));
    lines.push(build_batting_total(&result.home_batting, &cols));

    lines
}

// ---------------------------------------------------------------------------
// Advanced batting view
// ---------------------------------------------------------------------------

fn build_adv_batting_header() -> Line<'static> {
    let w = 7;
    let nw = 20;
    Line::from(vec![
        cell_left("Player", nw, HEADER_STYLE),
        cell("PA", 5, HEADER_STYLE),
        cell("wOBA", w, HEADER_STYLE),
        cell("ISO", w, HEADER_STYLE),
        cell("BABIP", w, HEADER_STYLE),
        cell("K%", w, HEADER_STYLE),
        cell("BB%", w, HEADER_STYLE),
        cell("QAB%", w, HEADER_STYLE),
        cell("P/PA", w, HEADER_STYLE),
        cell("GB%", w, HEADER_STYLE),
        cell("SB%", w, HEADER_STYLE),
    ])
}

fn build_adv_batting_row(p: &PlayerGameStats) -> Line<'static> {
    let b = &p.batting;
    let w = 7;
    let nw = 20;
    let woba_s = fmt_avg(b.woba);
    let iso_s = fmt_avg(b.iso);
    let babip_s = fmt_avg(b.babip);
    let k_pct_s = fmt_pct(b.k_pct);
    let bb_pct_s = fmt_pct(b.bb_pct);
    let qab_pct_s = fmt_pct(b.qab_pct);
    let p_pa_s = fmt_per(b.p_pa);
    let gb_pct_s = fmt_pct(b.gb_pct);
    let sb_pct_s = fmt_pct(b.sb_pct);
    Line::from(vec![
        cell_left(&p.player_id, nw, Style::default()),
        cell(&b.pa.to_string(), 5, int_style(b.pa)),
        cell(&woba_s, w, rate_style(b.woba)),
        cell(&iso_s, w, rate_style(b.iso)),
        cell(&babip_s, w, rate_style(b.babip)),
        cell(&k_pct_s, w, rate_style(b.k_pct)),
        cell(&bb_pct_s, w, rate_style(b.bb_pct)),
        cell(&qab_pct_s, w, rate_style(b.qab_pct)),
        cell(&p_pa_s, w, rate_style(b.p_pa)),
        cell(&gb_pct_s, w, rate_style(b.gb_pct)),
        cell(&sb_pct_s, w, rate_style(b.sb_pct)),
    ])
}

fn build_adv_batting_lines(
    result: &GameResult,
    away_team: &str,
    home_team: &str,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Away
    lines.push(Line::from(Span::styled(
        format!(" ADVANCED BATTING \u{2500} {away_team} "),
        TITLE_STYLE,
    )));
    lines.push(build_adv_batting_header());
    for p in team_batters(&result.player_stats, &result.away_id) {
        lines.push(build_adv_batting_row(p));
    }
    lines.push(Line::from(""));

    // Home
    lines.push(Line::from(Span::styled(
        format!(" ADVANCED BATTING \u{2500} {home_team} "),
        TITLE_STYLE,
    )));
    lines.push(build_adv_batting_header());
    for p in team_batters(&result.player_stats, &result.home_id) {
        lines.push(build_adv_batting_row(p));
    }

    lines
}

// ---------------------------------------------------------------------------
// Pitching view
// ---------------------------------------------------------------------------

fn build_pitching_header() -> Line<'static> {
    let w = 7;
    let nw = 20;
    Line::from(vec![
        cell_left("Player", nw, HEADER_STYLE),
        cell("IP", 6, HEADER_STYLE),
        cell("ERA", w, HEADER_STYLE),
        cell("FIP", w, HEADER_STYLE),
        cell("WHIP", w, HEADER_STYLE),
        cell("K/9", w, HEADER_STYLE),
        cell("BB/9", w, HEADER_STYLE),
        cell("K%", w, HEADER_STYLE),
        cell("K-BB%", w, HEADER_STYLE),
        cell("CSW%", w, HEADER_STYLE),
        cell("FPS%", w, HEADER_STYLE),
    ])
}

fn build_pitching_row(p: &PlayerGameStats) -> Line<'static> {
    let Some(ps) = &p.pitching else {
        return Line::from("");
    };
    let w = 7;
    let nw = 20;
    let ip_s = ps.ip_display.as_deref().unwrap_or("-").to_string();
    let era_s = fmt_rate(ps.era);
    let fip_s = fmt_rate(ps.fip);
    let whip_s = fmt_rate(ps.whip);
    let k9_s = fmt_per(ps.k9);
    let bb9_s = fmt_per(ps.bb9);
    let k_pct_s = fmt_pct(ps.k_pct);
    let k_bb_pct_s = fmt_pct(ps.k_bb_pct);
    let csw_pct_s = fmt_pct(ps.csw_pct);
    let fps_pct_s = fmt_pct(ps.fps_pct);
    Line::from(vec![
        cell_left(&p.player_id, nw, Style::default()),
        cell(&ip_s, 6, rate_style(ps.ip)),
        cell(&era_s, w, rate_style(ps.era)),
        cell(&fip_s, w, rate_style(ps.fip)),
        cell(&whip_s, w, rate_style(ps.whip)),
        cell(&k9_s, w, rate_style(ps.k9)),
        cell(&bb9_s, w, rate_style(ps.bb9)),
        cell(&k_pct_s, w, rate_style(ps.k_pct)),
        cell(&k_bb_pct_s, w, rate_style(ps.k_bb_pct)),
        cell(&csw_pct_s, w, rate_style(ps.csw_pct)),
        cell(&fps_pct_s, w, rate_style(ps.fps_pct)),
    ])
}

fn build_pitching_lines(
    result: &GameResult,
    away_team: &str,
    home_team: &str,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Away
    lines.push(Line::from(Span::styled(
        format!(" PITCHING \u{2500} {away_team} "),
        TITLE_STYLE,
    )));
    lines.push(build_pitching_header());
    for p in team_pitchers(&result.player_stats, &result.away_id) {
        lines.push(build_pitching_row(p));
    }
    lines.push(Line::from(""));

    // Home
    lines.push(Line::from(Span::styled(
        format!(" PITCHING \u{2500} {home_team} "),
        TITLE_STYLE,
    )));
    lines.push(build_pitching_header());
    for p in team_pitchers(&result.player_stats, &result.home_id) {
        lines.push(build_pitching_row(p));
    }

    lines
}

// ---------------------------------------------------------------------------
// Little League view
// ---------------------------------------------------------------------------

fn build_ll_team_lines(ll: &LittleLeagueStats, team_name: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    lines.push(Line::from(Span::styled(
        format!(" LITTLE LEAGUE \u{2500} {team_name} "),
        TITLE_STYLE,
    )));
    lines.push(Line::from(""));

    // Runs Breakdown
    lines.push(Line::from(Span::styled(
        " Runs Breakdown",
        HEADER_STYLE,
    )));
    let total_runs = ll.runs_on_bip + ll.runs_passive;
    let bip_run_pct = if total_runs > 0 {
        Some(f64::from(ll.runs_on_bip) / f64::from(total_runs))
    } else {
        None
    };
    lines.push(Line::from(format!(
        "   Runs on BIP:     {:>3}",
        ll.runs_on_bip
    )));
    lines.push(Line::from(format!(
        "   Passive Runs:    {:>3}    (BB/HBP/WP/PB)",
        ll.runs_passive
    )));
    lines.push(Line::from(format!(
        "   BIP Run %:     {:>5}",
        match bip_run_pct {
            Some(v) => format!("{:.1}%", v * 100.0),
            None => "-".to_string(),
        }
    )));
    lines.push(Line::from(""));

    // Pace
    lines.push(Line::from(Span::styled(" Pace", HEADER_STYLE)));
    let total_bip = ll.pitches_between_bip.len();
    let avg_pitches = if total_bip > 0 {
        let sum: i32 = ll.pitches_between_bip.iter().sum();
        Some(f64::from(sum) / total_bip as f64)
    } else {
        None
    };
    let min_pitches = ll.pitches_between_bip.iter().min().copied();
    let max_pitches = ll.pitches_between_bip.iter().max().copied();
    lines.push(Line::from(format!(
        "   Avg pitches between BIP:  {:>5}",
        match avg_pitches {
            Some(v) => format!("{v:.1}"),
            None => "-".to_string(),
        }
    )));
    lines.push(Line::from(format!(
        "   Min / Max:                {:>2} / {:<2}",
        min_pitches.map_or("-".to_string(), |v| v.to_string()),
        max_pitches.map_or("-".to_string(), |v| v.to_string()),
    )));
    lines.push(Line::from(format!(
        "   Total BIP:                {:>3}",
        total_bip
    )));
    lines.push(Line::from(""));

    // Baserunning Chaos
    lines.push(Line::from(Span::styled(
        " Baserunning Chaos",
        HEADER_STYLE,
    )));
    lines.push(Line::from(format!(
        "   Wild Pitches:    {:>3}",
        ll.wp
    )));
    lines.push(Line::from(format!(
        "   Passed Balls:    {:>3}",
        ll.pb
    )));
    lines.push(Line::from(format!(
        "   Caught Stealing: {:>3}",
        ll.cs
    )));
    lines.push(Line::from(format!(
        "   Steals of Home:  {:>3}",
        ll.steals_of_home
    )));
    lines.push(Line::from(""));

    lines
}

fn build_ll_lines(
    result: &GameResult,
    away_team: &str,
    home_team: &str,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.extend(build_ll_team_lines(&result.away_little_league, away_team));
    lines.extend(build_ll_team_lines(&result.home_little_league, home_team));
    lines
}

// ---------------------------------------------------------------------------
// Draw
// ---------------------------------------------------------------------------

fn draw_header(frame: &mut ratatui::Frame, area: Rect, current_view: View, game_name: &str) {
    let views = [View::BoxScore, View::Batting, View::Pitching, View::LittleLeague];
    let tab_spans: Vec<Span> = views
        .iter()
        .enumerate()
        .flat_map(|(i, v)| {
            let style = if *v == current_view {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                DIM_STYLE
            };
            let mut spans = Vec::new();
            if i > 0 {
                spans.push(Span::styled(" \u{2502} ", DIM_STYLE));
            }
            spans.push(Span::styled(format!(" {} ", v.label()), style));
            spans
        })
        .collect();

    let title_line = Line::from(vec![
        Span::styled("\u{25C7} diamond-replay", TITLE_STYLE),
        Span::styled(format!("  {game_name}  "), DIM_STYLE),
    ]);
    let tab_line = Line::from(tab_spans);
    let header = Paragraph::new(vec![title_line, tab_line]);
    frame.render_widget(header, area);
}

fn draw_body(frame: &mut ratatui::Frame, area: Rect, app: &mut App) {
    let content_lines = match app.view {
        View::BoxScore => &app.boxscore_content,
        View::Batting => &app.batting_content,
        View::Pitching => &app.pitching_content,
        View::LittleLeague => &app.ll_content,
    };

    let content_height = u16::try_from(content_lines.len()).unwrap_or(u16::MAX);
    app.viewport_height = area.height.saturating_sub(2); // block borders

    let max_scroll = content_height.saturating_sub(app.viewport_height);
    if app.scroll > max_scroll {
        app.scroll = max_scroll;
    }

    let body_block = Block::default()
        .borders(Borders::ALL)
        .border_style(DIM_STYLE)
        .padding(Padding::horizontal(1));

    let body = Paragraph::new(content_lines.clone())
        .block(body_block)
        .scroll((app.scroll, 0));

    frame.render_widget(body, area);

    if content_height > app.viewport_height {
        let mut scrollbar_state =
            ScrollbarState::new(usize::from(max_scroll)).position(usize::from(app.scroll));
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None);
        let scrollbar_area = Rect {
            x: area.x + area.width - 1,
            y: area.y + 1,
            width: 1,
            height: area.height.saturating_sub(2),
        };
        frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
    }
}

fn draw_footer(frame: &mut ratatui::Frame, area: Rect) {
    let footer_spans = vec![
        Span::styled("  q", HEADER_STYLE),
        Span::styled(": quit  ", DIM_STYLE),
        Span::styled("\u{2191}\u{2193}/jk", HEADER_STYLE),
        Span::styled(": scroll  ", DIM_STYLE),
        Span::styled("Tab", HEADER_STYLE),
        Span::styled(": switch view  ", DIM_STYLE),
        Span::styled("1/2/3/4", HEADER_STYLE),
        Span::styled(": jump to view", DIM_STYLE),
    ];
    let footer = Paragraph::new(Line::from(footer_spans));
    frame.render_widget(footer, area);
}

fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(frame.area());

    draw_header(frame, chunks[0], app.view, &app.game_name);
    draw_body(frame, chunks[1], app);
    draw_footer(frame, chunks[2]);
}

// ---------------------------------------------------------------------------
// TUI main loop
// ---------------------------------------------------------------------------

fn run_tui(result: &GameResult, game_name: String) -> io::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let backend = ratatui::backend::CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new(result, game_name);

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
        original_hook(info);
    }));

    loop {
        terminal.draw(|frame| draw(frame, &mut app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Tab => {
                    let next = app.view.next();
                    app.set_view(next);
                }
                KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
                KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
                KeyCode::Char('1') => app.set_view(View::BoxScore),
                KeyCode::Char('2') => app.set_view(View::Batting),
                KeyCode::Char('3') => app.set_view(View::Pitching),
                KeyCode::Char('4') => app.set_view(View::LittleLeague),
                _ => {}
            }
        }
    }

    let _ = std::panic::take_hook();
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// JSON output (--json mode)
// ---------------------------------------------------------------------------

fn pct1(num: i32, den: i32) -> f64 {
    if den == 0 { 0.0 } else { (f64::from(num) / f64::from(den) * 100.0 * 10.0).round() / 10.0 }
}

fn ratio1(num: i32, den: i32) -> f64 {
    if den == 0 { 0.0 } else { (f64::from(num) / f64::from(den) * 10.0).round() / 10.0 }
}

fn median_i32(v: &[i32]) -> f64 {
    if v.is_empty() { return 0.0; }
    let mut sorted = v.to_vec();
    sorted.sort_unstable();
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        f64::from(sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        f64::from(sorted[mid])
    }
}

/// Compute per-inning rate using outs recorded (handles partial innings).
fn per_inn(num: i32, outs: i32) -> f64 {
    if outs == 0 { return 0.0; }
    let ip = f64::from(outs) / 3.0;
    (f64::from(num) / ip * 10.0).round() / 10.0
}

/// Build the flat per-team stats object matching the stats-2026.html schema.
fn build_ll_team_json(
    bat: &BattingStats,
    own_pitch: &PitchingStats,
    opp_pitch: &PitchingStats,
    opp_bat: &BattingStats,
    opp_ll: &LittleLeagueStats,
    ll: &LittleLeagueStats,
    innings_bat: i32,
    innings_field: i32,
) -> serde_json::Value {
    use serde_json::{Map, Value, Number};

    let runs_total = bat.runs;
    let bip = opp_pitch.bip;
    let free_bases = bat.sb + bat.bb + bat.hbp + ll.wp + ll.pb;
    let def_free = opp_bat.sb + opp_bat.bb + opp_bat.hbp + opp_ll.wp + opp_ll.pb;
    // Use outs for accurate per-inning rates (handles partial innings)
    let outs_bat = opp_pitch.outs_recorded; // outs recorded against us = our batting outs
    let outs_field = own_pitch.outs_recorded;

    let mut m = Map::new();
    let i = |v: i32| Value::Number(Number::from(v));
    let f = |v: f64| Value::Number(Number::from_f64(v).unwrap_or(Number::from(0)));

    // Batting side
    m.insert("pitches".into(), i(opp_pitch.pitches));
    m.insert("balls".into(), i(opp_pitch.balls));
    m.insert("strikes_swinging".into(), i(opp_pitch.strikes_swinging));
    m.insert("strikes_looking".into(), i(opp_pitch.strikes_looking));
    m.insert("fouls".into(), i(opp_pitch.fouls));
    m.insert("bip".into(), i(bip));
    m.insert("hbp".into(), i(bat.hbp));
    m.insert("K".into(), i(bat.k));
    m.insert("K_looking".into(), i(bat.k_looking));
    m.insert("K_swinging".into(), i(bat.k_swinging));
    m.insert("BB".into(), i(bat.bb));
    m.insert("PA".into(), i(bat.pa));
    m.insert("sb".into(), i(bat.sb));
    m.insert("pb".into(), i(ll.pb));
    m.insert("wp".into(), i(ll.wp));
    m.insert("cs".into(), i(ll.cs));
    m.insert("steals_of_home".into(), i(ll.steals_of_home));
    m.insert("runs_on_bip".into(), i(ll.runs_on_bip));
    m.insert("runs_passive".into(), i(ll.runs_passive));
    m.insert("innings_bat".into(), i(innings_bat));
    m.insert("innings_field".into(), i(innings_field));

    // Batting rates
    m.insert("K_pct".into(), f(pct1(bat.k, bat.pa)));
    m.insert("K_looking_pct".into(), f(pct1(bat.k_looking, bat.k)));
    m.insert("K_swinging_pct".into(), f(pct1(bat.k_swinging, bat.k)));
    m.insert("BB_pct".into(), f(pct1(bat.bb, bat.pa)));
    m.insert("BIP_pct".into(), f(pct1(bip, bat.pa)));
    m.insert("HBP_pct".into(), f(pct1(bat.hbp, bat.pa)));
    m.insert("pitches_per_PA".into(), f(ratio1(opp_pitch.pitches, bat.pa)));
    m.insert("median_pitches_between_bip".into(), f(median_i32(&ll.pitches_between_bip)));
    m.insert("pitches_per_BIP".into(), f(ratio1(opp_pitch.pitches, bip)));
    m.insert("K_per_inn".into(), f(per_inn(bat.k, outs_bat)));
    m.insert("BB_per_inn".into(), f(per_inn(bat.bb, outs_bat)));
    m.insert("BIP_per_inn".into(), f(per_inn(bip, outs_bat)));
    m.insert("runs_total".into(), i(runs_total));
    m.insert("runs_on_bip_pct".into(), f(pct1(ll.runs_on_bip, runs_total)));
    m.insert("free_bases".into(), i(free_bases));
    m.insert("free_bases_per_inn".into(), f(per_inn(free_bases, outs_bat)));

    // Pitching side
    m.insert("pitch_pitches".into(), i(own_pitch.pitches));
    m.insert("pitch_balls".into(), i(own_pitch.balls));
    m.insert("pitch_strikes_sw".into(), i(own_pitch.strikes_swinging));
    m.insert("pitch_strikes_look".into(), i(own_pitch.strikes_looking));
    m.insert("pitch_fouls".into(), i(own_pitch.fouls));
    m.insert("pitch_bip".into(), i(own_pitch.bip));
    m.insert("pitch_ball_pct".into(), f(pct1(own_pitch.balls, own_pitch.pitches)));
    m.insert("pitch_strike_pct".into(), f(pct1(own_pitch.pitches - own_pitch.balls, own_pitch.pitches)));
    m.insert("pitch_K".into(), i(own_pitch.k));
    m.insert("pitch_BB".into(), i(own_pitch.bb));
    m.insert("pitch_K_per_inn".into(), f(per_inn(own_pitch.k, outs_field)));
    m.insert("pitch_BB_per_inn".into(), f(per_inn(own_pitch.bb, outs_field)));
    m.insert("pitch_BIP_per_inn".into(), f(per_inn(own_pitch.bip, outs_field)));
    m.insert("pitch_pitches_per_BIP".into(), f(ratio1(own_pitch.pitches, own_pitch.bip)));
    m.insert("pitch_median_p_between_bip".into(), f(median_i32(&ll.pitches_between_bip_pitching)));

    // Defense side
    m.insert("def_sb".into(), i(opp_bat.sb));
    m.insert("def_free_bases".into(), i(def_free));
    m.insert("def_free_bases_per_inn".into(), f(per_inn(def_free, outs_field)));

    Value::Object(m)
}

fn dump_json(result: &GameResult, game_name: &str, include_ll: bool) {
    let mut output = serde_json::json!({
        "name": game_name,
        "home_id": result.home_id,
        "away_id": result.away_id,
        "linescore_home": result.linescore_home,
        "linescore_away": result.linescore_away,
        "home_batting": result.home_batting,
        "away_batting": result.away_batting,
        "home_pitching": result.home_pitching,
        "away_pitching": result.away_pitching,
        "player_stats": result.player_stats,
        "transition_gaps": result.transition_gaps,
        "dead_time_per_inning": result.dead_time_per_inning,
        "first_timestamp": result.first_timestamp,
        "last_timestamp": result.last_timestamp,
        "duration_min": result.first_timestamp.zip(result.last_timestamp)
            .map(|(f, l)| f64::from(i32::try_from(l - f).unwrap_or(i32::MAX)) / 60_000.0),
    });
    if include_ll {
        let away_inn_bat = i32::try_from(result.linescore_away.len()).unwrap_or(0);
        let home_inn_bat = i32::try_from(result.linescore_home.len()).unwrap_or(0);

        let obj = output.as_object_mut().unwrap();
        obj.insert("teams".to_string(), serde_json::json!({
            result.away_id.clone(): build_ll_team_json(
                &result.away_batting, &result.away_pitching, &result.home_pitching,
                &result.home_batting, &result.home_little_league,
                &result.away_little_league,
                away_inn_bat, home_inn_bat,
            ),
            result.home_id.clone(): build_ll_team_json(
                &result.home_batting, &result.home_pitching, &result.away_pitching,
                &result.away_batting, &result.away_little_league,
                &result.home_little_league,
                home_inn_bat, away_inn_bat,
            ),
        }));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&output).expect("JSON serialization failed")
    );
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    let json_mode = args.iter().any(|a| a == "--json");
    let ll_flag = args.iter().any(|a| a == "--little-league");
    let paths: Vec<&String> = args
        .iter()
        .filter(|a| *a != "--json" && *a != "--little-league")
        .collect();

    // Read from file or stdin
    let is_stdin = paths.is_empty();
    let (data, game_name) = if is_stdin {
        // Reading from stdin — only works with --json (TUI needs a terminal)
        if !json_mode {
            eprintln!("Usage: diamond-replay <game.json> [--json] [--little-league]");
            eprintln!("       cat game.json | diamond-replay --json [--little-league]");
            process::exit(1);
        }
        let mut buf = String::new();
        io::Read::read_to_string(&mut io::stdin(), &mut buf).unwrap_or_else(|e| {
            eprintln!("Error reading stdin: {e}");
            process::exit(1);
        });
        (buf, "stdin".to_string())
    } else {
        let path = paths[0];
        let data = fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("Error reading {path}: {e}");
            process::exit(1);
        });
        let name = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(path)
            .to_string();
        (data, name)
    };

    let result = match replay_from_json(&data) {
        Ok(r) => r,
        Err(e) => {
            let source = if is_stdin { "stdin" } else { &game_name };
            eprintln!("Error replaying {source}: {e}");
            process::exit(1);
        }
    };

    if json_mode {
        dump_json(&result, &game_name, ll_flag);
    } else {
        run_tui(&result, game_name).unwrap_or_else(|e| {
            // Ensure terminal is restored on error
            let _ = disable_raw_mode();
            let _ = stdout().execute(LeaveAlternateScreen);
            eprintln!("TUI error: {e}");
            process::exit(1);
        });
    }
}
