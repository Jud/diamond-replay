use std::collections::HashMap;

use crate::error::{ReplayError, Result};
use crate::event::{
    attr_bool, attr_i32, attr_str, attr_usize, BipCause, BipPlayType, BrPlayType, EventData,
    PitchResult, PlayResult, RawApiEvent,
};
use crate::resolve;
use crate::score;
use crate::state::{BaseOccupant, GameState, PendingImplicit};
use crate::stats::RawStats;

/// Full result of replaying a game.
#[derive(Debug)]
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
    all_ts: Vec<i64>,
}

impl Replay {
    fn new() -> Self {
        Self {
            state: GameState::new(),
            half_stats: Vec::new(),
            runs_by_half: HashMap::new(),
            half_first_ts: HashMap::new(),
            half_last_ts: HashMap::new(),
            all_ts: Vec::new(),
        }
    }

    fn hi(&self) -> usize {
        self.state.half_inning
    }

    fn ensure_hi_stats(&mut self) {
        let hi = self.state.half_inning;
        score::ensure_stats(&mut self.half_stats, hi);
    }

    fn resolve(&mut self, discard: bool) {
        if let Some(pending) = self.state.pending.take() {
            let explicit = self.state.explicit_br_runners.clone();
            resolve::resolve_pending(
                self.hi(),
                &pending,
                &mut self.state.bases,
                &explicit,
                &mut self.runs_by_half,
                &mut self.half_stats,
            );
        }
        if discard {
            self.state.explicit_br_runners.clear();
        }
    }

    fn record_ts(&mut self, ts: i64) {
        self.all_ts.push(ts);
        let hi = self.hi();
        self.half_first_ts.entry(hi).or_insert(ts);
        self.half_last_ts.insert(hi, ts);
    }
}

// ---------------------------------------------------------------------------
// Pitch handling — each outcome is its own function
// ---------------------------------------------------------------------------

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
    if result != PitchResult::BallInPlay {
        r.resolve(false);
    }

    let hi = r.hi();
    r.ensure_hi_stats();
    r.half_stats[hi].pitches += 1;
    r.state.pitches_since_last_bip += 1;

    match result {
        PitchResult::Ball => {
            r.half_stats[hi].balls += 1;
            r.state.ball_count += 1;
            if r.state.ball_count >= 4 {
                r.half_stats[hi].bb += 1;
                r.half_stats[hi].pa += 1;
                score::force_advance_walk_score(
                    hi,
                    &r.state.bases,
                    &mut r.runs_by_half,
                    &mut r.half_stats,
                );
                score::apply_walk_bases(&mut r.state.bases);
                r.state.reset_count();
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
            r.resolve(false);
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
            score::force_advance_walk_score(
                hi,
                &r.state.bases,
                &mut r.runs_by_half,
                &mut r.half_stats,
            );
            score::apply_walk_bases(&mut r.state.bases);
            r.state.reset_count();
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
        if result == PitchResult::StrikeLooking {
            r.half_stats[hi].k_looking += 1;
        } else {
            r.half_stats[hi].k_swinging += 1;
        }
        r.half_stats[hi].pa += 1;
        r.state.outs += 1;
        r.state.reset_count();
        return r.state.outs >= 3;
    }
    false
}

// ---------------------------------------------------------------------------
// Ball-in-play handling
// ---------------------------------------------------------------------------

/// Returns true if this play caused a third out.
fn handle_ball_in_play(r: &mut Replay, attrs: &serde_json::Value) -> bool {
    r.state.reset_count();
    let pr = PlayResult::parse(attr_str(attrs, "playResult").unwrap_or(""));
    let hi = r.hi();
    r.ensure_hi_stats();

    if pr.is_dropped_third_strike() {
        r.half_stats[hi].k += 1;
        r.half_stats[hi].pa += 1;
        if r.state.last_strike_type.as_deref() == Some("strike_looking") {
            r.half_stats[hi].k_looking += 1;
        } else {
            r.half_stats[hi].k_swinging += 1;
        }
    }

    let snapshot = r.state.bases.snapshot();

    if pr.is_batter_out() {
        r.state.outs += 1;
        if pr == PlayResult::DoublPlay {
            r.state.outs += 1;
        }
        if pr.is_advance_runners_out() {
            r.state.pending = Some(PendingImplicit {
                play_result: pr,
                play_type: attr_str(attrs, "playType").map(BipPlayType::parse),
                cause: None,
                snapshot,
                outs_after_play: r.state.outs,
            });
            r.state.explicit_br_runners.clear();
        }
        return r.state.outs >= 3;
    }

    if pr == PlayResult::HomeRun {
        for b in [1, 2, 3] {
            if snapshot.is_occupied(b) {
                score::score_run(hi, &mut r.runs_by_half, &mut r.half_stats, true);
                r.state.bases.set(b, None);
            }
        }
        score::score_run(hi, &mut r.runs_by_half, &mut r.half_stats, true);
    } else if pr.sets_pending_implicit() {
        let (cause, play_type) = if pr == PlayResult::DroppedThirdStrike {
            (attr_str(attrs, "cause").map(BipCause::parse), None)
        } else {
            (None, attr_str(attrs, "playType").map(BipPlayType::parse))
        };
        r.state.pending = Some(PendingImplicit {
            play_result: pr,
            play_type,
            cause,
            snapshot,
            outs_after_play: r.state.outs,
        });
        r.state.explicit_br_runners.clear();
    }
    false
}

// ---------------------------------------------------------------------------
// Base-running handling
// ---------------------------------------------------------------------------

/// Returns true if this play caused a third out.
fn handle_base_running(r: &mut Replay, attrs: &serde_json::Value) -> bool {
    let pt = BrPlayType::parse(attr_str(attrs, "playType").unwrap_or(""));
    let base = attr_usize(attrs, "base");
    let runner_id = attr_str(attrs, "runnerId").map(String::from);

    if let Some(ref rid) = runner_id {
        r.state.explicit_br_runners.insert(rid.clone());
    }

    let hi = r.hi();
    r.ensure_hi_stats();
    record_br_stats(&mut r.half_stats[hi], pt, base);

    if pt.is_out() {
        r.state.outs += 1;
        if let (Some(ref rid), Some(b)) = (&runner_id, base) {
            r.state.bases.clear_runner(rid, b);
        }
        return r.state.outs >= 3;
    }

    if let (Some(b), Some(ref rid)) = (base, &runner_id) {
        if pt == BrPlayType::RemainedOnLastPlay && (1..=3).contains(&b) {
            update_remained(&mut r.state, b, rid);
        } else {
            r.state.bases.clear_runner(rid, b);
            if b == 4 {
                score::score_run(
                    hi,
                    &mut r.runs_by_half,
                    &mut r.half_stats,
                    pt.is_bip_advancement(),
                );
            } else if (1..=3).contains(&b) {
                r.state
                    .bases
                    .set(b, Some(BaseOccupant::Player(rid.clone())));
            }
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
    r.resolve(false);
    if attr_str(attrs, "reason") == Some("hit_by_pitch") {
        let hi = r.hi();
        r.ensure_hi_stats();
        r.half_stats[hi].hbp += 1;
        r.half_stats[hi].pa += 1;
        score::force_advance_walk_score(hi, &r.state.bases, &mut r.runs_by_half, &mut r.half_stats);
        score::apply_walk_bases(&mut r.state.bases);
        r.state.reset_count();
    }
}

fn handle_override(r: &mut Replay, attrs: &serde_json::Value) {
    r.resolve(false);

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
        score::apply_score_override(
            r.hi(),
            &r.state.home_id,
            &r.state.away_id,
            &mut r.runs_by_half,
            &mut r.half_stats,
            &entries,
        );
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
        first_timestamp: replay.all_ts.iter().copied().min(),
        last_timestamp: replay.all_ts.iter().copied().max(),
        transition_gaps: gaps,
        dead_time_per_inning: dead,
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
                "pitch" => handle_pitch(&mut r, &evt.attributes),
                "ball_in_play" => handle_ball_in_play(&mut r, &evt.attributes),
                "base_running" => handle_base_running(&mut r, &evt.attributes),
                "end_at_bat" => {
                    handle_end_at_bat(&mut r, &evt.attributes);
                    false
                }
                "end_half" => {
                    r.resolve(false);
                    true
                }
                "override" => {
                    handle_override(&mut r, &evt.attributes);
                    false
                }
                _ => false,
            };
        }

        if need_switch {
            r.resolve(false);
            r.state.do_switch();
        }
    }

    r.resolve(true);
    if !r.state.teams_set() {
        return Err(ReplayError::MissingTeams);
    }
    Ok(aggregate(r))
}
