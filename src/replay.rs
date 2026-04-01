use std::collections::HashMap;

use crate::error::{ReplayError, Result};
use crate::event::{
    attr_bool, attr_i32, attr_str, attr_usize, BipCause, BrPlayType, EventData, PitchResult,
    PlayResult, RawApiEvent,
};
use crate::player::{PlayerGameStats, PlayerTracker};
use crate::score;
use crate::state::{AutoAdvanceRecord, BaseOccupant, GameState};
use crate::stats::RawStats;

/// Full result of replaying a game.
#[derive(Debug, serde::Serialize)]
pub struct GameResult {
    pub home_id: String,
    pub away_id: String,
    pub linescore_away: Vec<i32>,
    pub linescore_home: Vec<i32>,
    pub away_batting: RawStats,
    pub home_batting: RawStats,
    pub away_halves_bat: i32,
    pub home_halves_bat: i32,
    pub first_timestamp: Option<i64>,
    pub last_timestamp: Option<i64>,
    pub transition_gaps: Vec<f64>,
    pub dead_time_per_inning: Vec<f64>,
    pub player_stats: HashMap<String, PlayerGameStats>,
}

// ---------------------------------------------------------------------------
// Replay context — owns all mutable state, delegates to focused handlers
// ---------------------------------------------------------------------------

struct Replay {
    state: GameState,
    half_stats: Vec<RawStats>,
    runs_by_half: HashMap<usize, i32>,
    half_first_ts: HashMap<usize, i64>,
    half_last_ts: HashMap<usize, i64>,
    first_ts: Option<i64>,
    last_ts: Option<i64>,
    players: PlayerTracker,
}

impl Replay {
    fn new() -> Self {
        Self {
            state: GameState::new(),
            half_stats: Vec::new(),
            runs_by_half: HashMap::new(),
            half_first_ts: HashMap::new(),
            half_last_ts: HashMap::new(),
            first_ts: None,
            last_ts: None,
            players: PlayerTracker::new(),
        }
    }

    /// The team currently fielding (not batting).
    fn defense_team(&self) -> &str {
        if self.state.offense == self.state.away_id {
            &self.state.home_id
        } else {
            &self.state.away_id
        }
    }

    fn hi(&self) -> usize {
        self.state.half_inning
    }

    fn ensure_hi_stats(&mut self) {
        let hi = self.state.half_inning;
        score::ensure_stats(&mut self.half_stats, hi);
    }

    fn record_ts(&mut self, ts: i64) {
        if self.first_ts.is_none() {
            self.first_ts = Some(ts);
        }
        self.last_ts = Some(ts);
        let hi = self.hi();
        self.half_first_ts.entry(hi).or_insert(ts);
        self.half_last_ts.insert(hi, ts);
    }
}

// ---------------------------------------------------------------------------
// Eager auto-advance helpers
// ---------------------------------------------------------------------------

/// Build a `BaseOccupant` for the batter: `Player(id)` when known, `Anonymous`
/// otherwise.
fn batter_occupant(batter_id: Option<&str>) -> BaseOccupant {
    match batter_id {
        Some(id) => BaseOccupant::Player(id.to_string()),
        None => BaseOccupant::Anonymous,
    }
}

/// Score a runner from the given base during auto-advance.
/// Records the run in `runs_by_half` / `half_stats`, records the player ID in
/// `record`, and records player stats.
fn auto_score(r: &mut Replay, base: usize, record: &mut AutoAdvanceRecord, on_bip: bool) {
    let hi = r.hi();
    let defense = r.defense_team().to_string();
    let pid = match r.state.bases.get(base) {
        Some(BaseOccupant::Player(id)) => Some(id.clone()),
        _ => None,
    };
    score::score_run(hi, &mut r.runs_by_half, &mut r.half_stats, on_bip);
    if let Some(ref id) = pid {
        r.players.record_run(id);
    }
    r.players.record_pitch_run(&defense);
    record.scored.push(pid);
    r.state.bases.set(base, None);
}

/// Apply eager auto-advance for a single (all runners advance 1 base).
fn auto_advance_single(r: &mut Replay, batter_id: Option<&str>) {
    let mut record = AutoAdvanceRecord::default();
    if r.state.bases.is_occupied(3) {
        auto_score(r, 3, &mut record, true);
    }
    if r.state.bases.is_occupied(2) {
        r.state.bases.advance(2, 3);
    }
    if r.state.bases.is_occupied(1) {
        r.state.bases.advance(1, 2);
    }
    r.state.bases.set(1, Some(batter_occupant(batter_id)));
    r.state.auto_advance = Some(record);
}

/// Apply eager auto-advance for a double (all runners advance 2 bases).
fn auto_advance_double(r: &mut Replay, batter_id: Option<&str>) {
    let mut record = AutoAdvanceRecord::default();
    if r.state.bases.is_occupied(3) {
        auto_score(r, 3, &mut record, true);
    }
    if r.state.bases.is_occupied(2) {
        auto_score(r, 2, &mut record, true);
    }
    if r.state.bases.is_occupied(1) {
        r.state.bases.advance(1, 3);
    }
    r.state.bases.set(2, Some(batter_occupant(batter_id)));
    r.state.auto_advance = Some(record);
}

/// Apply eager auto-advance for a triple (all runners advance 3 bases / score).
fn auto_advance_triple(r: &mut Replay, batter_id: Option<&str>) {
    let mut record = AutoAdvanceRecord::default();
    for b in [1, 2, 3] {
        if r.state.bases.is_occupied(b) {
            auto_score(r, b, &mut record, true);
        }
    }
    r.state.bases.set(3, Some(batter_occupant(batter_id)));
    r.state.bases.set(2, None);
    r.state.bases.set(1, None);
    r.state.auto_advance = Some(record);
}

/// Apply eager auto-advance for sacrifice fly / sacrifice bunt /
/// batter-out-advance-runners.
/// Score from 3B if runners behind, advance 2B->3B, advance 1B->2B.
fn auto_advance_advance_out(r: &mut Replay) {
    let mut record = AutoAdvanceRecord::default();
    let has_behind = r.state.bases.is_occupied(1) || r.state.bases.is_occupied(2);
    if has_behind && r.state.bases.is_occupied(3) {
        auto_score(r, 3, &mut record, true);
    }
    if r.state.bases.is_occupied(2) {
        r.state.bases.advance(2, 3);
    }
    if r.state.bases.is_occupied(1) {
        r.state.bases.advance(1, 2);
    }
    r.state.auto_advance = Some(record);
}

/// Apply eager auto-advance for a dropped third strike.
fn auto_advance_dropped_third(r: &mut Replay, cause: Option<BipCause>, batter_id: Option<&str>) {
    let mut record = AutoAdvanceRecord::default();
    if cause.is_some_and(BipCause::is_ball_away) {
        // Wild pitch / passed ball: all runners advance one base
        if r.state.bases.is_occupied(3) {
            auto_score(r, 3, &mut record, false);
        }
        if r.state.bases.is_occupied(2) {
            r.state.bases.advance(2, 3);
        }
        if r.state.bases.is_occupied(1) {
            r.state.bases.advance(1, 2);
        }
        r.state.bases.set(1, Some(batter_occupant(batter_id)));
    } else {
        // Force-advance walk: if bases loaded, runner from 3B scores
        if r.state.bases.is_occupied(1)
            && r.state.bases.is_occupied(2)
            && r.state.bases.is_occupied(3)
        {
            auto_score(r, 3, &mut record, false);
        }
        score::apply_walk_bases(&mut r.state.bases, batter_id);
    }
    r.state.auto_advance = Some(record);
}

/// Check if a runner was auto-scored (present in the auto-advance record).
fn was_auto_scored(state: &GameState, runner_id: &str) -> bool {
    state.auto_advance.as_ref().is_some_and(|rec| {
        rec.scored
            .iter()
            .any(|pid| pid.as_deref() == Some(runner_id))
    })
}

/// Undo an auto-scored run for a runner: decrement the run counter and
/// the player/pitcher stats.
fn undo_auto_scored_run(r: &mut Replay, runner_id: &str, on_bip: bool) {
    let hi = r.hi();
    let defense = r.defense_team().to_string();
    score::undo_score_run(hi, &mut r.runs_by_half, &mut r.half_stats, on_bip);
    r.players.undo_run(runner_id);
    r.players.undo_pitch_run(&defense);
}

// ---------------------------------------------------------------------------
// Pitch handling — each outcome is its own function
// ---------------------------------------------------------------------------

/// Score from bases-loaded walk/HBP, recording the runner who scored.
fn record_walk_run(r: &mut Replay, hi: usize, defense: &str) {
    // Capture the runner at 3B before the bases change
    let runner_3b = if r.state.bases.is_occupied(1)
        && r.state.bases.is_occupied(2)
        && r.state.bases.is_occupied(3)
    {
        match r.state.bases.get(3) {
            Some(BaseOccupant::Player(id)) => Some(id.clone()),
            _ => None,
        }
    } else {
        None
    };
    let scored =
        score::force_advance_walk_score(hi, &r.state.bases, &mut r.runs_by_half, &mut r.half_stats);
    if scored {
        if let Some(ref pid) = runner_3b {
            r.players.record_run(pid);
        }
        r.players.record_pitch_run(defense);
    }
}

fn handle_set_teams(r: &mut Replay, attrs: &serde_json::Value) {
    r.state.home_id = attr_str(attrs, "homeId").unwrap_or("").to_string();
    r.state.away_id = attr_str(attrs, "awayId").unwrap_or("").to_string();
    r.state.offense = r.state.away_id.clone();
}

/// Returns true if the pitch caused a third out.
fn handle_pitch(r: &mut Replay, attrs: &serde_json::Value) -> bool {
    if !attr_bool(attrs, "advancesCount", true) {
        if let Some(res) = attr_str(attrs, "result") {
            if res == "strike_swinging" || res == "strike_looking" {
                r.state.last_strike_type = Some(res.to_string());
            }
        }
        return false;
    }

    let result = PitchResult::parse(attr_str(attrs, "result").unwrap_or(""));

    let hi = r.hi();
    let offense = r.state.offense.clone();
    let defense = r.defense_team().to_string();
    let batter_id = r.players.current_batter(&offense).map(str::to_string);
    r.ensure_hi_stats();
    r.half_stats[hi].pitches += 1;
    r.state.pitches_since_last_bip += 1;

    // Record pitch for pitcher stats
    let is_ball = result == PitchResult::Ball;
    r.players.record_pitch_thrown(&defense, is_ball);

    match result {
        PitchResult::Ball => {
            r.half_stats[hi].balls += 1;
            r.state.ball_count += 1;
            if r.state.ball_count >= 4 {
                r.half_stats[hi].bb += 1;
                r.half_stats[hi].pa += 1;
                record_walk_run(r, hi, &defense);
                score::apply_walk_bases(&mut r.state.bases, batter_id.as_deref());
                r.state.reset_count();
                r.players.record_bb(&offense);
                r.players.record_pitch_bb(&defense);
            }
            false
        }
        PitchResult::StrikeSwinging | PitchResult::StrikeLooking => {
            handle_strike(r, hi, result, attrs)
        }
        PitchResult::Foul => {
            r.half_stats[hi].fouls += 1;
            if r.state.strike_count < 2 {
                r.state.strike_count += 1;
            }
            false
        }
        PitchResult::BallInPlay => {
            r.ensure_hi_stats();
            let psbip = r.state.pitches_since_last_bip;
            r.half_stats[hi].bip += 1;
            r.half_stats[hi].pa += 1;
            r.half_stats[hi].pitches_between_bip.push(psbip);
            r.state.pitches_since_last_bip = 0;
            r.state.reset_count();
            false
        }
        PitchResult::HitByPitch => {
            r.half_stats[hi].hbp += 1;
            r.half_stats[hi].pa += 1;
            record_walk_run(r, hi, &defense);
            score::apply_walk_bases(&mut r.state.bases, batter_id.as_deref());
            r.state.reset_count();
            r.players.record_hbp(&offense);
            r.players.record_pitch_hbp(&defense);
            false
        }
        PitchResult::Unknown => false,
    }
}

/// Returns true if this strikeout caused a third out.
fn handle_strike(
    r: &mut Replay,
    hi: usize,
    result: PitchResult,
    attrs: &serde_json::Value,
) -> bool {
    if result == PitchResult::StrikeSwinging {
        r.half_stats[hi].strikes_swinging += 1;
    } else {
        r.half_stats[hi].strikes_looking += 1;
    }
    r.state.strike_count += 1;
    r.state.last_strike_type = Some(attr_str(attrs, "result").unwrap_or("").to_string());

    if r.state.strike_count >= 3 {
        r.half_stats[hi].k += 1;
        let looking = result == PitchResult::StrikeLooking;
        if looking {
            r.half_stats[hi].k_looking += 1;
        } else {
            r.half_stats[hi].k_swinging += 1;
        }
        r.half_stats[hi].pa += 1;
        r.state.outs += 1;
        r.state.reset_count();
        let offense = r.state.offense.clone();
        let defense = r.defense_team().to_string();
        r.players.record_k(&offense, looking);
        r.players.record_pitch_k(&defense);
        return r.state.outs >= 3;
    }
    false
}

// ---------------------------------------------------------------------------
// Ball-in-play handling (eager auto-advance)
// ---------------------------------------------------------------------------

/// Returns true if this play caused a third out.
fn handle_ball_in_play(r: &mut Replay, attrs: &serde_json::Value) -> bool {
    // Clear any previous auto-advance record before applying a new one
    r.state.auto_advance = None;
    r.state.reset_count();

    let pr = PlayResult::parse(attr_str(attrs, "playResult").unwrap_or(""));
    let offense = r.state.offense.clone();
    let defense = r.defense_team().to_string();
    let batter_id = r.players.current_batter(&offense).map(str::to_string);
    r.ensure_hi_stats();

    if pr.is_dropped_third_strike() {
        let hi = r.hi();
        let looking = r.state.last_strike_type.as_deref() == Some("strike_looking");
        r.half_stats[hi].k += 1;
        r.half_stats[hi].pa += 1;
        if looking {
            r.half_stats[hi].k_looking += 1;
        } else {
            r.half_stats[hi].k_swinging += 1;
        }
        r.players.record_dropped_k(&offense, looking);
        r.players.record_pitch_k(&defense);
    } else if !pr.is_batter_out() {
        // Record the hit type for batter and pitcher
        r.players.record_bip(&offense, pr);
        if matches!(
            pr,
            PlayResult::Single | PlayResult::Double | PlayResult::Triple | PlayResult::HomeRun
        ) {
            r.players.record_pitch_hit(&defense, pr);
        }
    } else {
        // Batter out on a batted ball — still a PA, advance the lineup
        r.players.record_bip(&offense, pr);
    }

    if pr.is_batter_out() {
        r.state.outs += 1;
        if pr == PlayResult::DoublPlay {
            r.state.outs += 1;
        }
        if pr.is_advance_runners_out() && r.state.outs < 3 {
            auto_advance_advance_out(r);
        }
        return r.state.outs >= 3;
    }

    let hi = r.hi();
    if pr == PlayResult::HomeRun {
        // Home runs are already eager — score everyone plus the batter
        for b in [1, 2, 3] {
            if r.state.bases.is_occupied(b) {
                score::score_run(hi, &mut r.runs_by_half, &mut r.half_stats, true);
                if let Some(BaseOccupant::Player(id)) = r.state.bases.get(b) {
                    r.players.record_run(id);
                }
                r.players.record_pitch_run(&defense);
                r.state.bases.set(b, None);
            }
        }
        // Batter scores — use pre-captured ID (lineup already advanced by record_bip)
        score::score_run(hi, &mut r.runs_by_half, &mut r.half_stats, true);
        if let Some(ref bid) = batter_id {
            r.players.record_run(bid);
        }
        r.players.record_pitch_run(&defense);
    } else {
        // Apply eager auto-advance based on the play result
        match pr {
            PlayResult::Single | PlayResult::Error | PlayResult::FieldersChoice => {
                auto_advance_single(r, batter_id.as_deref());
            }
            PlayResult::Double => auto_advance_double(r, batter_id.as_deref()),
            PlayResult::Triple => auto_advance_triple(r, batter_id.as_deref()),
            PlayResult::DroppedThirdStrike => {
                let cause = attr_str(attrs, "cause").map(BipCause::parse);
                auto_advance_dropped_third(r, cause, batter_id.as_deref());
            }
            _ => {}
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Base-running handling (works against post-auto-advance state)
// ---------------------------------------------------------------------------

/// Returns true if this play caused a third out.
fn handle_base_running(r: &mut Replay, attrs: &serde_json::Value) -> bool {
    let pt = BrPlayType::parse(attr_str(attrs, "playType").unwrap_or(""));
    let base = attr_usize(attrs, "base");
    let runner_id = attr_str(attrs, "runnerId").map(String::from);

    let hi = r.hi();
    let defense = r.defense_team().to_string();
    r.ensure_hi_stats();
    record_br_stats(&mut r.half_stats[hi], pt, base);

    // Record player baserunning stats
    if let (Some(ref rid), Some(b)) = (&runner_id, base) {
        r.players.record_baserunning(rid, pt, b);
        if b == 4 && !pt.is_out() {
            r.players.record_pitch_run(&defense);
        }
    }

    if pt.is_out() {
        if let (Some(ref rid), Some(b)) = (&runner_id, base) {
            let on_bases = r.state.bases.find_by_id(rid).is_some();
            if on_bases {
                r.state.outs += 1;
                r.state.bases.clear_runner(rid, b);
            } else if was_auto_scored(&r.state, rid) {
                // Runner was auto-scored but actually got out — undo the run
                undo_auto_scored_run(r, rid, true);
                r.state.outs += 1;
            } else {
                r.state.outs += 1;
            }
        } else {
            r.state.outs += 1;
        }
        return r.state.outs >= 3;
    }

    if let (Some(b), Some(ref rid)) = (base, &runner_id) {
        let occupant = Some(BaseOccupant::Player(rid.clone()));
        let on_bases = r.state.bases.find_by_id(rid).is_some();

        if pt == BrPlayType::RemainedOnLastPlay && (1..=3).contains(&b) {
            if on_bases {
                update_remained(&mut r.state, b, rid);
            } else {
                if was_auto_scored(&r.state, rid) {
                    undo_auto_scored_run(r, rid, true);
                }
                r.state.bases.set(b, occupant);
            }
        } else if b == 4 {
            if on_bases {
                r.state.bases.clear_runner(rid, b);
                score::score_run(
                    hi,
                    &mut r.runs_by_half,
                    &mut r.half_stats,
                    pt.is_bip_advancement(),
                );
            }
            // Not on bases (already auto-scored) → confirmation, skip
        } else if (1..=3).contains(&b) {
            if on_bases {
                r.state.bases.clear_by_id(rid);
            } else if was_auto_scored(&r.state, rid) {
                undo_auto_scored_run(r, rid, true);
            } else {
                r.state.bases.clear_runner(rid, b);
            }
            r.state.bases.set(b, occupant);
        }
    }
    false
}

fn record_br_stats(s: &mut RawStats, pt: BrPlayType, base: Option<usize>) {
    match pt {
        BrPlayType::StoleBase => {
            s.sb += 1;
            if base == Some(4) {
                s.steals_of_home += 1;
            }
        }
        BrPlayType::PassedBall => {
            s.pb += 1;
            if base == Some(4) {
                s.steals_of_home += 1;
            }
        }
        BrPlayType::WildPitch => {
            s.wp += 1;
            if base == Some(4) {
                s.steals_of_home += 1;
            }
        }
        BrPlayType::CaughtStealing => {
            s.cs += 1;
        }
        _ => {}
    }
}

/// Runner didn't move — update tracking from anonymous to their real ID.
fn update_remained(state: &mut GameState, base: usize, rid: &str) {
    for ob in 1..=3 {
        if ob != base {
            if let Some(BaseOccupant::Player(pid)) = state.bases.get(ob) {
                if pid == rid {
                    state.bases.set(ob, None);
                }
            }
        }
    }
    state
        .bases
        .set(base, Some(BaseOccupant::Player(rid.to_string())));
}

// ---------------------------------------------------------------------------
// Override + end-of-at-bat handlers
// ---------------------------------------------------------------------------

fn handle_end_at_bat(r: &mut Replay, attrs: &serde_json::Value) {
    let reason = attr_str(attrs, "reason").unwrap_or("");
    if reason == "hit_by_pitch" || reason == "catcher_interference" {
        let hi = r.hi();
        let offense = r.state.offense.clone();
        let defense = r.defense_team().to_string();
        let batter_id = r.players.current_batter(&offense).map(str::to_string);
        r.ensure_hi_stats();
        r.half_stats[hi].hbp += 1;
        r.half_stats[hi].pa += 1;
        record_walk_run(r, hi, &defense);
        score::apply_walk_bases(&mut r.state.bases, batter_id.as_deref());
        r.state.reset_count();
        r.players.record_hbp(&offense);
        r.players.record_pitch_hbp(&defense);
    }
}

/// Compute the total runs for a team from `runs_by_half`.
fn team_run_total(runs_by_half: &HashMap<usize, i32>, half_inning: usize, parity: usize) -> i32 {
    (parity..=half_inning)
        .step_by(2)
        .map(|hi| runs_by_half.get(&hi).copied().unwrap_or(0))
        .sum()
}

fn handle_override(r: &mut Replay, attrs: &serde_json::Value) {
    if let Some(scores_arr) = attrs.get("scores").and_then(|s| s.as_array()) {
        let entries: Vec<score::ScoreOverrideEntry> = scores_arr
            .iter()
            .filter_map(|s| {
                let score_val = i32::try_from(s.get("score")?.as_i64()?).ok()?;
                Some(score::ScoreOverrideEntry {
                    team_id: s.get("teamId")?.as_str()?.to_string(),
                    score: score_val,
                })
            })
            .collect();

        // Capture per-team run totals before the override
        let hi = r.hi();
        let away_before = team_run_total(&r.runs_by_half, hi, 0);
        let home_before = team_run_total(&r.runs_by_half, hi, 1);

        score::apply_score_override(
            hi,
            &r.state.home_id,
            &r.state.away_id,
            &mut r.runs_by_half,
            &mut r.half_stats,
            &entries,
        );

        // Check if runs were reduced and adjust player stats accordingly
        let away_after = team_run_total(&r.runs_by_half, hi, 0);
        let home_after = team_run_total(&r.runs_by_half, hi, 1);
        let away_id = r.state.away_id.clone();
        let home_id = r.state.home_id.clone();
        if away_after < away_before {
            r.players
                .adjust_team_runs(&away_id, away_after - away_before);
        }
        if home_after < home_before {
            r.players
                .adjust_team_runs(&home_id, home_after - home_before);
        }
    }

    if let Some(half) = attr_str(attrs, "half") {
        let is_top = r.state.half_inning.is_multiple_of(2);
        if (half == "bottom" && is_top) || (half == "top" && !is_top) {
            r.state.do_switch();
        }
    }
    if let Some(o) = attr_i32(attrs, "outs") {
        r.state.outs = o;
    }
    if let Some(b) = attr_i32(attrs, "balls") {
        r.state.ball_count = b;
    }
    if let Some(s) = attr_i32(attrs, "strikes") {
        r.state.strike_count = s;
    }
}

// ---------------------------------------------------------------------------
// Post-replay aggregation
// ---------------------------------------------------------------------------

fn aggregate(replay: Replay) -> GameResult {
    let player_stats = replay.players.into_stats();
    let num_hi = replay.half_stats.len();
    let mut away_batting = RawStats::new();
    let mut home_batting = RawStats::new();
    let (mut away_halves, mut home_halves) = (0i32, 0i32);

    for (i, hs) in replay.half_stats.iter().enumerate() {
        if i % 2 == 0 {
            away_batting.merge(hs);
            away_halves += 1;
        } else {
            home_batting.merge(hs);
            home_halves += 1;
        }
    }

    let gaps = build_transition_gaps(num_hi, &replay.half_first_ts, &replay.half_last_ts);
    let dead = build_dead_time(&gaps);

    GameResult {
        home_id: replay.state.home_id,
        away_id: replay.state.away_id,
        linescore_away: (0..num_hi.div_ceil(2))
            .map(|i| replay.runs_by_half.get(&(i * 2)).copied().unwrap_or(0))
            .collect(),
        linescore_home: (0..num_hi / 2)
            .map(|i| replay.runs_by_half.get(&(i * 2 + 1)).copied().unwrap_or(0))
            .collect(),
        away_batting,
        home_batting,
        away_halves_bat: away_halves,
        home_halves_bat: home_halves,
        first_timestamp: replay.first_ts,
        last_timestamp: replay.last_ts,
        transition_gaps: gaps,
        dead_time_per_inning: dead,
        player_stats,
    }
}

fn build_transition_gaps(
    n: usize,
    first: &HashMap<usize, i64>,
    last: &HashMap<usize, i64>,
) -> Vec<f64> {
    (0..n.saturating_sub(1))
        .map(|hi| match (last.get(&hi), first.get(&(hi + 1))) {
            (Some(&e), Some(&s)) if s > e => {
                f64::from(i32::try_from(s - e).unwrap_or(i32::MAX)) / 1000.0
            }
            _ => 0.0,
        })
        .collect()
}

fn build_dead_time(gaps: &[f64]) -> Vec<f64> {
    gaps.chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| c[0] + c[1])
        .collect()
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Replay a game from undo-resolved events.
///
/// # Errors
///
/// Returns an error if the event list is empty, teams are not set,
/// or any event data fails to parse as JSON.
pub fn replay_game(resolved: &[RawApiEvent]) -> Result<GameResult> {
    if resolved.is_empty() {
        return Err(ReplayError::NoEvents);
    }

    let mut r = Replay::new();

    for raw in resolved {
        let ed: EventData = serde_json::from_str(&raw.event_data).map_err(ReplayError::Json)?;
        let sub_events = match ed {
            EventData::Transaction { events, .. } => events,
            EventData::Single(e) => vec![e],
        };

        let mut need_switch = false;
        for evt in &sub_events {
            if let Some(ts) = evt.created_at {
                r.record_ts(ts);
            }
            need_switch |= match evt.code.as_str() {
                "set_teams" => {
                    handle_set_teams(&mut r, &evt.attributes);
                    false
                }
                "fill_lineup_index" => {
                    if let (Some(tid), Some(pid), Some(idx)) = (
                        attr_str(&evt.attributes, "teamId"),
                        attr_str(&evt.attributes, "playerId"),
                        attr_usize(&evt.attributes, "index"),
                    ) {
                        r.players.handle_fill_lineup(tid, pid, idx);
                    }
                    false
                }
                "fill_lineup" => {
                    if let (Some(tid), Some(pid)) = (
                        attr_str(&evt.attributes, "teamId"),
                        attr_str(&evt.attributes, "playerId"),
                    ) {
                        r.players.handle_fill_lineup_roster(tid, pid);
                    }
                    false
                }
                "fill_position" => {
                    if let (Some(tid), Some(pid), Some(pos)) = (
                        attr_str(&evt.attributes, "teamId"),
                        attr_str(&evt.attributes, "playerId"),
                        attr_str(&evt.attributes, "position"),
                    ) {
                        r.players.handle_fill_position(tid, pid, pos);
                    }
                    false
                }
                "goto_lineup_index" => {
                    if let (Some(tid), Some(idx)) = (
                        attr_str(&evt.attributes, "teamId"),
                        attr_usize(&evt.attributes, "index"),
                    ) {
                        r.players.handle_goto(tid, idx);
                    }
                    false
                }
                "pitch" => handle_pitch(&mut r, &evt.attributes),
                "ball_in_play" => handle_ball_in_play(&mut r, &evt.attributes),
                "base_running" => handle_base_running(&mut r, &evt.attributes),
                "end_at_bat" => {
                    handle_end_at_bat(&mut r, &evt.attributes);
                    false
                }
                "end_half" => true,
                "override" => {
                    handle_override(&mut r, &evt.attributes);
                    false
                }
                _ => false,
            };
        }

        if need_switch {
            r.state.do_switch();
        }
    }

    if !r.state.teams_set() {
        return Err(ReplayError::MissingTeams);
    }
    Ok(aggregate(r))
}
