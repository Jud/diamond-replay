use std::collections::HashMap;

use crate::compute;
use crate::error::{ReplayError, Result};
use crate::event::{
    attr_bool, attr_i32, attr_str, attr_usize, BipCause, BipPlayType, BrPlayType, EventData,
    PitchResult, PlayResult, RawApiEvent,
};
use crate::player::{BattingStats, PitchingStats, PlayerGameStats, PlayerTracker};
use crate::score;
use crate::state::{AutoAdvanceRecord, BaseOccupant, GameState};

/// Team-level metrics especially relevant for youth/little league baseball.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct LittleLeagueStats {
    /// Runs scored as a result of a ball being put in play (hits, errors, FC, sac flies, etc.)
    pub runs_on_bip: i32,
    /// Runs scored without a batted ball (BB w/ bases loaded, HBP w/ bases loaded, WP, PB, balk)
    pub runs_passive: i32,
    /// Number of pitches between each ball in play
    pub pitches_between_bip: Vec<i32>,
    /// Wild pitches
    pub wp: i32,
    /// Passed balls
    pub pb: i32,
    /// Caught stealing
    pub cs: i32,
    /// Steals of home plate
    pub steals_of_home: i32,
    /// Number of pitches between each ball in play (pitching/defense perspective)
    pub pitches_between_bip_pitching: Vec<i32>,
    /// Bases-loaded walks
    pub bb_loaded: i32,
    /// Bases-loaded HBP
    pub hbp_loaded: i32,
}

/// Full result of replaying a game.
#[derive(Debug, serde::Serialize)]
pub struct GameResult {
    pub home_id: String,
    pub away_id: String,
    pub linescore_away: Vec<i32>,
    pub linescore_home: Vec<i32>,
    pub first_timestamp: Option<i64>,
    pub last_timestamp: Option<i64>,
    pub transition_gaps: Vec<f64>,
    pub dead_time_per_inning: Vec<f64>,
    pub player_stats: HashMap<String, PlayerGameStats>,
    pub away_batting: BattingStats,
    pub home_batting: BattingStats,
    pub away_pitching: PitchingStats,
    pub home_pitching: PitchingStats,
    pub away_little_league: LittleLeagueStats,
    pub home_little_league: LittleLeagueStats,
}

// ---------------------------------------------------------------------------
// Replay context -- owns all mutable state, delegates to focused handlers
// ---------------------------------------------------------------------------

struct Replay {
    state: GameState,
    runs_by_half: HashMap<usize, i32>,
    /// Track the max half-inning index seen (for linescore construction).
    max_half_inning: usize,
    half_first_ts: HashMap<usize, i64>,
    half_last_ts: HashMap<usize, i64>,
    first_ts: Option<i64>,
    last_ts: Option<i64>,
    players: PlayerTracker,
    away_ll: LittleLeagueStats,
    home_ll: LittleLeagueStats,
    pitches_since_last_bip_away: i32,
    pitches_since_last_bip_home: i32,
    /// Pitching-side: pitches thrown since last BIP (away team pitching)
    pitch_pitches_since_last_bip_away: i32,
    /// Pitching-side: pitches thrown since last BIP (home team pitching)
    pitch_pitches_since_last_bip_home: i32,
    /// Pending BIP run snapshot: (parity, runs_before, is_away_batting, is_real_bip)
    /// is_real_bip=false for dropped third strikes (runs go to passive instead).
    pending_bip_snapshot: Option<(usize, i32, bool, bool)>,
}

impl Replay {
    fn new() -> Self {
        Self {
            state: GameState::new(),
            runs_by_half: HashMap::new(),
            max_half_inning: 0,
            half_first_ts: HashMap::new(),
            half_last_ts: HashMap::new(),
            first_ts: None,
            last_ts: None,
            players: PlayerTracker::new(),
            away_ll: LittleLeagueStats::default(),
            home_ll: LittleLeagueStats::default(),
            pitches_since_last_bip_away: 0,
            pitches_since_last_bip_home: 0,
            pitch_pitches_since_last_bip_away: 0,
            pitch_pitches_since_last_bip_home: 0,
            pending_bip_snapshot: None,
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

    fn track_hi(&mut self) {
        let hi = self.state.half_inning;
        if hi > self.max_half_inning {
            self.max_half_inning = hi;
        }
    }

    /// Return the `LittleLeagueStats` for the team currently batting.
    fn ll_for_offense(&mut self) -> &mut LittleLeagueStats {
        if self.state.offense == self.state.away_id {
            &mut self.away_ll
        } else {
            &mut self.home_ll
        }
    }

    /// Increment pitches-since-last-BIP for both batting and pitching sides.
    fn inc_pitches_since_last_bip(&mut self) {
        if self.state.offense == self.state.away_id {
            self.pitches_since_last_bip_away += 1;
            self.pitch_pitches_since_last_bip_home += 1;
        } else {
            self.pitches_since_last_bip_home += 1;
            self.pitch_pitches_since_last_bip_away += 1;
        }
    }

    /// Push current count to pitches_between_bip and reset for both batting and pitching sides.
    fn flush_pitches_between_bip(&mut self) {
        if self.state.offense == self.state.away_id {
            self.away_ll.pitches_between_bip.push(self.pitches_since_last_bip_away);
            self.pitches_since_last_bip_away = 0;
            self.home_ll.pitches_between_bip_pitching.push(self.pitch_pitches_since_last_bip_home);
            self.pitch_pitches_since_last_bip_home = 0;
        } else {
            self.home_ll.pitches_between_bip.push(self.pitches_since_last_bip_home);
            self.pitches_since_last_bip_home = 0;
            self.away_ll.pitches_between_bip_pitching.push(self.pitch_pitches_since_last_bip_away);
            self.pitch_pitches_since_last_bip_away = 0;
        }
    }

    /// Resolve the pending BIP run snapshot after all corrections in the
    /// transaction have been applied. The delta between runs_before and
    /// runs_after (post-correction) is the true number of runs scored.
    /// Uses the stored team identity so half-inning switches don't misattribute.
    fn resolve_bip_snapshot(&mut self) {
        if let Some((parity, runs_before, is_away, is_real_bip)) = self.pending_bip_snapshot.take() {
            let hi = self.hi();
            let runs_after = team_run_total(&self.runs_by_half, hi, parity);
            let delta = runs_after - runs_before;
            if delta > 0 {
                let ll = if is_away { &mut self.away_ll } else { &mut self.home_ll };
                if is_real_bip {
                    ll.runs_on_bip += delta;
                } else {
                    // Dropped third strike — runs are passive (WP/PB/force)
                    ll.runs_passive += delta;
                }
            }
        }
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
/// Records the run in `runs_by_half`, records the player ID in
/// `record`, and records player stats.
fn auto_score(r: &mut Replay, base: usize, record: &mut AutoAdvanceRecord) {
    let hi = r.hi();
    let defense = r.defense_team().to_string();
    let pid = match r.state.bases.get(base) {
        Some(BaseOccupant::Player(id)) => Some(id.clone()),
        _ => None,
    };
    score::score_run(hi, &mut r.runs_by_half);
    if let Some(ref id) = pid {
        r.players.record_run(id);
        // Track earned runs: if runner is NOT error-tagged, it's an earned run
        if !r.state.error_runners.contains(id) {
            r.players.record_pitch_earned_run(&defense);
        }
    } else {
        // Anonymous runner: assume earned
        r.players.record_pitch_earned_run(&defense);
    }
    r.players.record_pitch_run(&defense);
    record.scored.push(pid.clone());
    r.state.bases.set(base, None);
    if let Some(ref id) = pid {
        if r.state.error_runners.remove(id) {
            record.error_tagged.insert(id.clone());
        }
    }
}

/// Apply eager auto-advance for a single (all runners advance 1 base).
fn auto_advance_single(r: &mut Replay, batter_id: Option<&str>) {
    let mut record = AutoAdvanceRecord::default();
    if r.state.bases.is_occupied(3) {
        auto_score(r, 3, &mut record);
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
        auto_score(r, 3, &mut record);
    }
    if r.state.bases.is_occupied(2) {
        auto_score(r, 2, &mut record);
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
            auto_score(r, b, &mut record);
        }
    }
    r.state.bases.set(3, Some(batter_occupant(batter_id)));
    r.state.bases.set(2, None);
    r.state.bases.set(1, None);
    r.state.auto_advance = Some(record);
}

/// Apply eager auto-advance for sacrifice fly / sacrifice bunt /
/// batter-out-advance-runners.
/// Score from 3B, advance 2B->3B, advance 1B->2B.
fn auto_advance_advance_out(r: &mut Replay) {
    let mut record = AutoAdvanceRecord::default();
    if r.state.bases.is_occupied(3) {
        auto_score(r, 3, &mut record);
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
    if cause.is_some_and(BipCause::is_ball_away) {
        // Wild pitch / passed ball: all runners advance one base (same as single)
        auto_advance_single(r, batter_id);
    } else {
        // Force-advance walk: if bases loaded, runner from 3B scores
        let mut record = AutoAdvanceRecord::default();
        if r.state.bases.is_occupied(1)
            && r.state.bases.is_occupied(2)
            && r.state.bases.is_occupied(3)
        {
            auto_score(r, 3, &mut record);
        }
        score::apply_walk_bases(&mut r.state.bases, batter_id);
        r.state.auto_advance = Some(record);
    }
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
fn undo_auto_scored_run(r: &mut Replay, runner_id: &str) {
    let hi = r.hi();
    let defense = r.defense_team().to_string();
    score::undo_score_run(hi, &mut r.runs_by_half);
    r.players.undo_run(runner_id);
    r.players.undo_pitch_run(&defense);
    let was_error = r
        .state
        .auto_advance
        .as_ref()
        .is_some_and(|rec| rec.error_tagged.contains(runner_id));
    if was_error {
        r.state.error_runners.insert(runner_id.to_string());
    } else {
        r.players.undo_pitch_earned_run(&defense);
    }
    // If no BIP snapshot is pending, this undo is happening in a separate
    // transaction from the BIP that auto-scored. Adjust LL runs_on_bip
    // directly since the snapshot delta already counted it.
    if r.pending_bip_snapshot.is_none() {
        let ll = r.ll_for_offense();
        ll.runs_on_bip -= 1;
        if ll.runs_on_bip < 0 {
            ll.runs_on_bip = 0;
        }
    }
}

// ---------------------------------------------------------------------------
// PA context + QAB helpers
// ---------------------------------------------------------------------------

/// Determine if a completed PA qualifies as a Quality At-Bat.
fn is_qab(pr: PlayResult, ctx: &crate::state::PAContext) -> bool {
    // Hit
    if matches!(
        pr,
        PlayResult::Single | PlayResult::Double | PlayResult::Triple | PlayResult::HomeRun
    ) {
        return true;
    }
    // BB, HBP, SF, SAC, ROE
    if matches!(
        pr,
        PlayResult::Error | PlayResult::SacrificeFly | PlayResult::SacrificeBunt
    ) {
        return true;
    }
    // 3+ pitches after reaching two strikes
    if ctx.pitches_after_two_strikes >= 3 {
        return true;
    }
    // 6+ total pitches in the PA
    if ctx.pitches_in_pa >= 6 {
        return true;
    }
    false
}

/// Determine if a completed PA qualifies as a competitive AB.
/// A competitive AB is one where the batter reached a 2-strike count.
fn is_competitive(ctx: &crate::state::PAContext) -> bool {
    ctx.reached_two_strikes
}

// ---------------------------------------------------------------------------
// Pitch handling -- each outcome is its own function
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
    let scored = score::force_advance_walk_score(hi, &r.state.bases, &mut r.runs_by_half);
    if scored {
        if let Some(ref pid) = runner_3b {
            r.players.record_run(pid);
            // Earned run tracking
            if !r.state.error_runners.contains(pid) {
                r.players.record_pitch_earned_run(defense);
            }
            r.state.error_runners.remove(pid);
        } else {
            // Anonymous runner: assume earned
            r.players.record_pitch_earned_run(defense);
        }
        r.players.record_pitch_run(defense);
    }
}

/// Why the batter reached base without putting the ball in play.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ReachCause {
    Walk,
    HitByPitch,
    CatcherInterference,
}

/// Common logic for completing a walk, HBP, or catcher interference PA.
/// Handles: walk run scoring, base advancement, PA context, QAB, competitive AB, RBI.
fn complete_walk_or_hbp(
    r: &mut Replay,
    offense: &str,
    defense: &str,
    batter_id: Option<&str>,
    cause: ReachCause,
) {
    let hi = r.hi();
    let bases_loaded = r.state.bases.is_occupied(1)
        && r.state.bases.is_occupied(2)
        && r.state.bases.is_occupied(3);
    record_walk_run(r, hi, defense);
    // Track passive run + loaded walk/HBP for LL stats
    if bases_loaded {
        let ll = r.ll_for_offense();
        ll.runs_passive += 1;
        match cause {
            ReachCause::Walk => ll.bb_loaded += 1,
            ReachCause::HitByPitch => ll.hbp_loaded += 1,
            ReachCause::CatcherInterference => {} // neither
        }
    }
    score::apply_walk_bases(&mut r.state.bases, batter_id);
    r.players
        .record_pa_context(offense, defense, &r.state.pa_context);
    r.players.record_pitch_bf(defense);
    r.players.record_qab(offense);
    if is_competitive(&r.state.pa_context) {
        r.players.record_competitive_ab(offense);
    }
    if bases_loaded {
        if let Some(bid) = batter_id {
            r.players.record_rbi(bid, 1);
        }
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

    let offense = r.state.offense.clone();
    let defense = r.defense_team().to_string();
    let batter_id = r.players.current_batter(&offense).map(str::to_string);
    r.track_hi();

    // Track PA context
    if r.state.pa_context.pitches_in_pa == 0 {
        let is_strike = matches!(
            result,
            PitchResult::StrikeSwinging
                | PitchResult::StrikeLooking
                | PitchResult::Foul
                | PitchResult::BallInPlay
        );
        r.state.pa_context.first_pitch_strike = is_strike;
    }
    r.state.pa_context.pitches_in_pa += 1;
    if r.state.pa_context.reached_two_strikes {
        r.state.pa_context.pitches_after_two_strikes += 1;
    }

    // Record pitch for pitcher stats
    r.players.record_pitch_thrown(&defense, result);

    // Track pitches between BIP for Little League stats
    r.inc_pitches_since_last_bip();

    match result {
        PitchResult::Ball => {
            r.state.ball_count += 1;
            if r.state.ball_count >= 4 {
                complete_walk_or_hbp(r, &offense, &defense, batter_id.as_deref(), ReachCause::Walk);
                r.state.reset_count();
                r.players.record_bb(&offense);
                r.players.record_pitch_bb(&defense);
            }
            false
        }
        PitchResult::StrikeSwinging | PitchResult::StrikeLooking => handle_strike(r, result, attrs),
        PitchResult::Foul => {
            if r.state.strike_count < 2 {
                r.state.strike_count += 1;
            }
            if r.state.strike_count >= 2 {
                r.state.pa_context.reached_two_strikes = true;
            }
            false
        }
        PitchResult::BallInPlay => {
            r.state.reset_count();
            false
        }
        PitchResult::HitByPitch => {
            complete_walk_or_hbp(r, &offense, &defense, batter_id.as_deref(), ReachCause::HitByPitch);
            r.state.reset_count();
            r.players.record_hbp(&offense);
            r.players.record_pitch_hbp(&defense);
            false
        }
        PitchResult::Unknown => false,
    }
}

/// Returns true if this strikeout caused a third out.
fn handle_strike(r: &mut Replay, result: PitchResult, attrs: &serde_json::Value) -> bool {
    r.state.strike_count += 1;
    r.state.last_strike_type = Some(attr_str(attrs, "result").unwrap_or("").to_string());

    // Track 2-strike status
    if r.state.strike_count >= 2 {
        r.state.pa_context.reached_two_strikes = true;
    }

    if r.state.strike_count >= 3 {
        let looking = result == PitchResult::StrikeLooking;
        let offense = r.state.offense.clone();
        let defense = r.defense_team().to_string();
        // PA context before reset
        r.players
            .record_pa_context(&offense, &defense, &r.state.pa_context);
        r.players.record_pitch_bf(&defense);
        if is_competitive(&r.state.pa_context) {
            r.players.record_competitive_ab(&offense);
        }
        // QAB: K is not a QAB unless pitches criteria met
        if r.state.pa_context.pitches_after_two_strikes >= 3
            || r.state.pa_context.pitches_in_pa >= 6
        {
            r.players.record_qab(&offense);
        }
        r.state.outs += 1;
        r.state.reset_count();
        r.players.record_k(&offense, looking);
        r.players.record_pitch_k(&defense);
        r.players.record_pitch_out(&defense);
        return r.state.outs >= 3;
    }
    false
}

// ---------------------------------------------------------------------------
// Ball-in-play handling (eager auto-advance)
// ---------------------------------------------------------------------------

/// Record batting/pitching stats for a BIP and classify as QAB if appropriate.
fn record_bip_stats(
    r: &mut Replay,
    pr: PlayResult,
    bip_type: BipPlayType,
    offense: &str,
    defense: &str,
    batter_id: Option<&str>,
    pa_is_qab: bool,
) {
    if pr.is_dropped_third_strike() {
        let looking = r.state.last_strike_type.as_deref() == Some("strike_looking");
        r.players.record_k(offense, looking);
        r.players.record_pitch_k(defense);
        if r.state.pa_context.pitches_after_two_strikes >= 3
            || r.state.pa_context.pitches_in_pa >= 6
        {
            r.players.record_qab(offense);
        }
    } else if !pr.is_batter_out() {
        r.players.record_bip(offense, pr, bip_type);
        if pr.is_hit() {
            r.players.record_pitch_hit(defense, pr, bip_type);
        } else {
            r.players.record_pitch_bip(defense, bip_type);
        }
        if pa_is_qab {
            r.players.record_qab(offense);
        }
        if pr == PlayResult::Error {
            if let Some(bid) = batter_id {
                r.state.error_runners.insert(bid.to_string());
            }
        }
    } else {
        r.players.record_bip(offense, pr, bip_type);
        r.players.record_pitch_bip(defense, bip_type);
        if pa_is_qab {
            r.players.record_qab(offense);
        }
    }
}

/// Score a home run: clear all bases, score all runners + batter, credit RBI.
fn score_home_run(r: &mut Replay, defense: &str, batter_id: Option<&str>) {
    let hi = r.hi();
    let mut rbi_count: i32 = 0;
    let mut scored_ids: Vec<String> = Vec::new();
    for b in [1, 2, 3] {
        if r.state.bases.is_occupied(b) {
            score::score_run(hi, &mut r.runs_by_half);
            if let Some(BaseOccupant::Player(id)) = r.state.bases.get(b) {
                r.players.record_run(id);
                if !r.state.error_runners.contains(id) {
                    r.players.record_pitch_earned_run(defense);
                }
                scored_ids.push(id.clone());
            } else {
                r.players.record_pitch_earned_run(defense);
            }
            r.players.record_pitch_run(defense);
            r.state.bases.set(b, None);
            rbi_count += 1;
        }
    }
    for id in &scored_ids {
        r.state.error_runners.remove(id);
    }
    score::score_run(hi, &mut r.runs_by_half);
    if let Some(bid) = batter_id {
        r.players.record_run(bid);
    }
    r.players.record_pitch_run(defense);
    r.players.record_pitch_earned_run(defense);
    rbi_count += 1;
    if let Some(bid) = batter_id {
        r.players.record_rbi(bid, rbi_count);
    }
}

/// Returns true if this play caused a third out.
fn handle_ball_in_play(r: &mut Replay, attrs: &serde_json::Value) -> bool {
    r.state.auto_advance = None;

    let pr = PlayResult::parse(attr_str(attrs, "playResult").unwrap_or(""));
    let bip_type = attr_str(attrs, "playType").map_or(BipPlayType::Other, BipPlayType::parse);
    let offense = r.state.offense.clone();
    let defense = r.defense_team().to_string();
    let batter_id = r.players.current_batter(&offense).map(str::to_string);
    r.track_hi();

    let is_dropped_third = pr.is_dropped_third_strike();
    let is_away = offense == r.state.away_id;
    let parity = if is_away { 0 } else { 1 };
    let runs_before = team_run_total(&r.runs_by_half, r.hi(), parity);

    // Always snapshot so we can categorize any runs scored during this event.
    // For real BIP: runs go to runs_on_bip. For dropped thirds: runs_passive.
    r.pending_bip_snapshot = Some((parity, runs_before, is_away, !is_dropped_third));

    if !is_dropped_third {
        r.flush_pitches_between_bip();
    }

    r.players
        .record_pa_context(&offense, &defense, &r.state.pa_context);
    r.players.record_pitch_bf(&defense);

    let pa_is_qab = is_qab(pr, &r.state.pa_context);
    let pa_is_competitive = is_competitive(&r.state.pa_context);

    r.state.reset_count();

    record_bip_stats(
        r,
        pr,
        bip_type,
        &offense,
        &defense,
        batter_id.as_deref(),
        pa_is_qab,
    );

    if pa_is_competitive {
        r.players.record_competitive_ab(&offense);
    }

    let result = if pr.is_batter_out() {
        handle_bip_out(r, pr, &offense, &defense, batter_id.as_deref())
    } else if pr == PlayResult::HomeRun {
        score_home_run(r, &defense, batter_id.as_deref());
        false
    } else {
        apply_bip_advance(r, pr, attrs, batter_id.as_deref());
        false
    };

    result
}

/// Handle a batter-out on a BIP. Returns true if third out.
fn handle_bip_out(
    r: &mut Replay,
    pr: PlayResult,
    _offense: &str,
    defense: &str,
    batter_id: Option<&str>,
) -> bool {
    r.state.outs += 1;
    r.players.record_pitch_out(defense);
    if pr == PlayResult::DoublPlay {
        r.state.outs += 1;
        r.players.record_pitch_out(defense);
        if let Some(bid) = batter_id {
            r.players.record_gidp(bid);
        }
    }
    if pr.is_advance_runners_out() && r.state.outs < 3 {
        auto_advance_advance_out(r);
        if let (Some(bid), Some(ref aa)) = (batter_id, &r.state.auto_advance) {
            let rbi_count = i32::try_from(aa.scored.len()).unwrap_or(0);
            if rbi_count > 0 {
                r.players.record_rbi(bid, rbi_count);
            }
        }
    }

    // Auto-score runners held at 3B by the no-steal-home filter.
    // In the real game the runner had already scored, so the scorer
    // didn't tag this out as a sac fly / productive out. Score them
    // on any BIP that doesn't end the inning.
    if r.state.outs < 3 && !r.state.held_at_third.is_empty() && r.state.bases.is_occupied(3) {
        if let Some(occ) = r.state.bases.get(3).clone() {
            let rid = match &occ {
                crate::state::BaseOccupant::Player(id) => Some(id.clone()),
                crate::state::BaseOccupant::Anonymous => None,
            };
            if rid.as_ref().is_some_and(|id| r.state.held_at_third.contains(id)) {
                auto_score(r, 3, &mut AutoAdvanceRecord::default());
                if let Some(id) = &rid {
                    r.state.held_at_third.remove(id);
                }
            }
        }
    }

    r.state.outs >= 3
}

/// Apply auto-advance for non-HR, non-out BIP results and credit RBI.
fn apply_bip_advance(
    r: &mut Replay,
    pr: PlayResult,
    attrs: &serde_json::Value,
    batter_id: Option<&str>,
) {
    match pr {
        PlayResult::Single | PlayResult::Error | PlayResult::FieldersChoice => {
            auto_advance_single(r, batter_id);
        }
        PlayResult::Double => auto_advance_double(r, batter_id),
        PlayResult::Triple => auto_advance_triple(r, batter_id),
        PlayResult::DroppedThirdStrike => {
            let cause = attr_str(attrs, "cause").map(BipCause::parse);
            auto_advance_dropped_third(r, cause, batter_id);
        }
        _ => {}
    }
    if pr != PlayResult::Error {
        if let (Some(bid), Some(ref aa)) = (batter_id, &r.state.auto_advance) {
            let rbi_count = i32::try_from(aa.scored.len()).unwrap_or(0);
            if rbi_count > 0 {
                r.players.record_rbi(bid, rbi_count);
            }
        }
    }
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
    r.track_hi();

    // Record player baserunning stats
    if let (Some(ref rid), Some(b)) = (&runner_id, base) {
        r.players.record_baserunning(rid, pt, b);
        if b == 4 && !pt.is_out() {
            r.players.record_pitch_run(&defense);
            // Earned run tracking
            if !r.state.error_runners.contains(rid) {
                r.players.record_pitch_earned_run(&defense);
            }
            r.state.error_runners.remove(rid);
        }
    }

    // Track WP for pitcher
    if pt == BrPlayType::WildPitch {
        r.players.record_pitch_wp(&defense);
    }

    // LL: track WP, PB, CS, steals of home, passive runs
    match pt {
        BrPlayType::WildPitch => {
            r.ll_for_offense().wp += 1;
            if base == Some(4) && !pt.is_out() {
                if runner_id.as_ref().is_some_and(|rid| r.state.bases.find_by_id(rid).is_some()) {
                    r.ll_for_offense().runs_passive += 1;
                }
            }
        }
        BrPlayType::PassedBall => {
            r.ll_for_offense().pb += 1;
            if base == Some(4) && !pt.is_out() {
                if runner_id.as_ref().is_some_and(|rid| r.state.bases.find_by_id(rid).is_some()) {
                    r.ll_for_offense().runs_passive += 1;
                }
            }
        }
        BrPlayType::CaughtStealing => {
            r.ll_for_offense().cs += 1;
        }
        BrPlayType::StoleBase => {
            if base == Some(4) {
                // Only count if runner is actually on bases (not already auto-scored)
                if runner_id.as_ref().is_some_and(|rid| r.state.bases.find_by_id(rid).is_some()) {
                    r.ll_for_offense().steals_of_home += 1;
                    r.ll_for_offense().runs_passive += 1;
                }
            }
        }
        BrPlayType::DefensiveIndifference | BrPlayType::OnSamePitch | BrPlayType::OtherAdvance => {
            // Passive run only if runner is still on bases (not already auto-scored)
            if base == Some(4) && !pt.is_out() {
                if runner_id.as_ref().is_some_and(|rid| r.state.bases.find_by_id(rid).is_some()) {
                    r.ll_for_offense().runs_passive += 1;
                }
            }
        }
        BrPlayType::AdvancedOnLastPlay | BrPlayType::AdvancedOnError | BrPlayType::OnSameError => {
            // BIP run — only count if runner is still on bases (not already
            // auto-scored) AND there's no pending BIP snapshot (which would
            // capture this run via the delta). If there IS a pending snapshot,
            // the delta will handle it when resolve_bip_snapshot runs.
            if base == Some(4) && !pt.is_out() && r.pending_bip_snapshot.is_none() {
                if runner_id.as_ref().is_some_and(|rid| r.state.bases.find_by_id(rid).is_some()) {
                    r.ll_for_offense().runs_on_bip += 1;
                }
            }
        }
        _ => {}
    }

    if pt.is_out() {
        if let (Some(ref rid), Some(b)) = (&runner_id, base) {
            let on_bases = r.state.bases.find_by_id(rid).is_some();
            if on_bases {
                r.state.outs += 1;
                r.players.record_pitch_out(&defense);
                r.state.bases.clear_runner(rid, b);
            } else if was_auto_scored(&r.state, rid) {
                // Runner was auto-scored but actually got out -- undo the run
                undo_auto_scored_run(r, rid);
                r.state.outs += 1;
                r.players.record_pitch_out(&defense);
            } else {
                r.state.outs += 1;
                r.players.record_pitch_out(&defense);
            }
            r.state.error_runners.remove(rid);
        } else {
            r.state.outs += 1;
            r.players.record_pitch_out(&defense);
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
                    undo_auto_scored_run(r, rid);
                }
                r.state.bases.set(b, occupant);
            }
        } else if b == 4 {
            if on_bases {
                r.state.bases.clear_runner(rid, b);
                score::score_run(hi, &mut r.runs_by_half);
            }
            // Not on bases (already auto-scored) -> confirmation, skip
        } else if (1..=3).contains(&b) {
            if on_bases {
                r.state.bases.clear_by_id(rid);
            } else if was_auto_scored(&r.state, rid) {
                undo_auto_scored_run(r, rid);
            } else {
                r.state.bases.clear_runner(rid, b);
            }
            r.state.bases.set(b, occupant);
        }
    }
    false
}

/// Runner didn't move -- update tracking from anonymous to their real ID.
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
        let offense = r.state.offense.clone();
        let defense = r.defense_team().to_string();
        let batter_id = r.players.current_batter(&offense).map(str::to_string);
        let cause = if reason == "hit_by_pitch" {
            ReachCause::HitByPitch
        } else {
            ReachCause::CatcherInterference
        };
        r.track_hi();
        complete_walk_or_hbp(r, &offense, &defense, batter_id.as_deref(), cause);
        r.state.reset_count();
        if reason == "hit_by_pitch" {
            r.players.record_hbp(&offense);
            r.players.record_pitch_hbp(&defense);
        }
        // Catcher interference: batter reaches base but it's not an HBP.
        // PA context and base advancement are handled by complete_walk_or_hbp.
        // No HBP stat credit.
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
            &entries,
        );

        // Check if runs changed and adjust player stats + LL totals accordingly
        let away_after = team_run_total(&r.runs_by_half, hi, 0);
        let home_after = team_run_total(&r.runs_by_half, hi, 1);
        let away_id = r.state.away_id.clone();
        let home_id = r.state.home_id.clone();
        let away_delta = away_after - away_before;
        let home_delta = home_after - home_before;
        if away_delta < 0 {
            r.players.adjust_team_runs(&away_id, away_delta);
        }
        if home_delta < 0 {
            r.players.adjust_team_runs(&home_id, home_delta);
        }
        // Adjust LL run totals to stay in sync with the official linescore.
        // Skip if a BIP snapshot is pending — resolve_bip_snapshot will
        // capture the post-override delta, so adjusting here would double-count.
        if r.pending_bip_snapshot.is_none() {
            if away_delta != 0 {
                r.away_ll.runs_on_bip += away_delta;
                if r.away_ll.runs_on_bip < 0 {
                    r.away_ll.runs_passive += r.away_ll.runs_on_bip;
                    r.away_ll.runs_on_bip = 0;
                }
            }
            if home_delta != 0 {
                r.home_ll.runs_on_bip += home_delta;
                if r.home_ll.runs_on_bip < 0 {
                    r.home_ll.runs_passive += r.home_ll.runs_on_bip;
                    r.home_ll.runs_on_bip = 0;
                }
            }
        }
        // Adjust pitcher runs_allowed / earned_runs_allowed for the fielding team
        if away_delta != 0 {
            r.players.adjust_pitch_runs(&home_id, away_delta);
        }
        if home_delta != 0 {
            r.players.adjust_pitch_runs(&away_id, home_delta);
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

/// Compute all derived fields (integer + rate) for a `BattingStats` in place.
fn fill_batting_derived(b: &mut BattingStats) {
    let raw = compute::BattingRaw {
        pa: b.pa,
        singles: b.singles,
        doubles: b.doubles,
        triples: b.triples,
        home_runs: b.home_runs,
        bb: b.bb,
        hbp: b.hbp,
        k: b.k,
        sac_fly: b.sac_fly,
        sac_bunt: b.sac_bunt,
        sb: b.sb,
        cs: b.cs,
        ground_balls: b.ground_balls,
        fly_balls: b.fly_balls,
        line_drives: b.line_drives,
        pop_ups: b.pop_ups,
        pitches_seen: b.pitches_seen,
        qab: b.qab,
        competitive_ab: b.competitive_ab,
        hard_hit_balls: b.hard_hit_balls,
    };
    let d = compute::compute_batting(&raw, &compute::DEFAULT_WOBA_WEIGHTS);
    b.ab = d.ab;
    b.hits = d.hits;
    b.tb = d.tb;
    b.xbh = d.xbh;
    b.avg = d.avg;
    b.obp = d.obp;
    b.slg = d.slg;
    b.ops = d.ops;
    b.iso = d.iso;
    b.babip = d.babip;
    b.k_pct = d.k_pct;
    b.bb_pct = d.bb_pct;
    b.bb_k = d.bb_k;
    b.woba = d.woba;
    b.gb_pct = d.gb_pct;
    b.fb_pct = d.fb_pct;
    b.ld_pct = d.ld_pct;
    b.hr_fb = d.hr_fb;
    b.p_pa = d.p_pa;
    b.qab_pct = d.qab_pct;
    b.competitive_pct = d.competitive_pct;
    b.hard_hit_pct = d.hard_hit_pct;
    b.sb_pct = d.sb_pct;
}

/// Compute all derived fields for a `PitchingStats` in place.
fn fill_pitching_derived(p: &mut PitchingStats) {
    let raw = compute::PitchingRaw {
        outs_recorded: p.outs_recorded,
        hits_allowed: p.hits_allowed,
        hr_allowed: p.hr_allowed,
        bb: p.bb,
        hbp: p.hbp,
        k: p.k,
        earned_runs_allowed: p.earned_runs_allowed,
        runs_allowed: p.runs_allowed,
        bf: p.bf,
        pitches: p.pitches,
        strikes_swinging: p.strikes_swinging,
        strikes_looking: p.strikes_looking,
        first_pitch_strikes: p.first_pitch_strikes,
        fouls: p.fouls,
        ground_balls: p.ground_balls,
        fly_balls: p.fly_balls,
        line_drives: p.line_drives,
        pop_ups: p.pop_ups,
        bip: p.bip,
    };
    let d = compute::compute_pitching(&raw, &compute::DEFAULT_FIP_CONSTANTS);
    p.ip = Some(d.ip);
    p.ip_display = Some(d.ip_display);
    p.era = d.era;
    p.whip = d.whip;
    p.k9 = d.k9;
    p.bb9 = d.bb9;
    p.h9 = d.h9;
    p.hr9 = d.hr9;
    p.k_bb = d.k_bb;
    p.fip = d.fip;
    p.k_pct = d.k_pct;
    p.bb_pct = d.bb_pct;
    p.k_bb_pct = d.k_bb_pct;
    p.babip = d.babip;
    p.hr_fb = d.hr_fb;
    p.gb_pct = d.gb_pct;
    p.fb_pct = d.fb_pct;
    p.ld_pct = d.ld_pct;
    p.sw_str_pct = d.sw_str_pct;
    p.csw_pct = d.csw_pct;
    p.c_str_pct = d.c_str_pct;
    p.fps_pct = d.fps_pct;
    p.foul_pct = d.foul_pct;
    p.game_score = Some(d.game_score);
    p.pitches_per_ip = d.pitches_per_ip;
}

/// Add raw counts from `src` into `dst` (team-level aggregation).
fn merge_batting(dst: &mut BattingStats, src: &BattingStats) {
    dst.pa += src.pa;
    dst.k += src.k;
    dst.k_looking += src.k_looking;
    dst.k_swinging += src.k_swinging;
    dst.bb += src.bb;
    dst.hbp += src.hbp;
    dst.singles += src.singles;
    dst.doubles += src.doubles;
    dst.triples += src.triples;
    dst.home_runs += src.home_runs;
    dst.sac_fly += src.sac_fly;
    dst.sac_bunt += src.sac_bunt;
    dst.fc += src.fc;
    dst.roe += src.roe;
    dst.gidp += src.gidp;
    dst.rbi += src.rbi;
    dst.runs += src.runs;
    dst.ground_balls += src.ground_balls;
    dst.fly_balls += src.fly_balls;
    dst.line_drives += src.line_drives;
    dst.pop_ups += src.pop_ups;
    dst.hard_hit_balls += src.hard_hit_balls;
    dst.pitches_seen += src.pitches_seen;
    dst.qab += src.qab;
    dst.competitive_ab += src.competitive_ab;
    dst.sb += src.sb;
    dst.cs += src.cs;
}

/// Add raw counts from `src` into `dst` (team-level aggregation).
fn merge_pitching(dst: &mut PitchingStats, src: &PitchingStats) {
    dst.pitches += src.pitches;
    dst.balls += src.balls;
    dst.strikes_swinging += src.strikes_swinging;
    dst.strikes_looking += src.strikes_looking;
    dst.fouls += src.fouls;
    dst.k += src.k;
    dst.bb += src.bb;
    dst.hbp += src.hbp;
    dst.hits_allowed += src.hits_allowed;
    dst.hr_allowed += src.hr_allowed;
    dst.runs_allowed += src.runs_allowed;
    dst.earned_runs_allowed += src.earned_runs_allowed;
    dst.outs_recorded += src.outs_recorded;
    dst.bf += src.bf;
    dst.bip += src.bip;
    dst.ground_balls += src.ground_balls;
    dst.fly_balls += src.fly_balls;
    dst.line_drives += src.line_drives;
    dst.pop_ups += src.pop_ups;
    dst.first_pitch_strikes += src.first_pitch_strikes;
    dst.wp += src.wp;
}

fn aggregate(replay: Replay) -> GameResult {
    let mut player_stats = replay.players.into_stats();
    let num_hi = replay.max_half_inning + 1;

    // Compute derived fields (integer + rate) for each player
    for ps in player_stats.values_mut() {
        fill_batting_derived(&mut ps.batting);
        if let Some(ref mut p) = ps.pitching {
            fill_pitching_derived(p);
        }
    }

    // Aggregate team batting and pitching stats from per-player data
    let mut away_batting = BattingStats::default();
    let mut home_batting = BattingStats::default();
    let mut away_pitching = PitchingStats::default();
    let mut home_pitching = PitchingStats::default();

    for ps in player_stats.values() {
        if ps.team_id == replay.state.away_id {
            merge_batting(&mut away_batting, &ps.batting);
            if let Some(ref p) = ps.pitching {
                merge_pitching(&mut away_pitching, p);
            }
        } else if ps.team_id == replay.state.home_id {
            merge_batting(&mut home_batting, &ps.batting);
            if let Some(ref p) = ps.pitching {
                merge_pitching(&mut home_pitching, p);
            }
        }
    }

    // Compute derived fields for team totals
    fill_batting_derived(&mut away_batting);
    fill_batting_derived(&mut home_batting);
    fill_pitching_derived(&mut away_pitching);
    fill_pitching_derived(&mut home_pitching);

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
        first_timestamp: replay.first_ts,
        last_timestamp: replay.last_ts,
        transition_gaps: gaps,
        dead_time_per_inning: dead,
        player_stats,
        away_batting,
        home_batting,
        away_pitching,
        home_pitching,
        away_little_league: replay.away_ll,
        home_little_league: replay.home_ll,
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
pub fn replay_game(resolved: &[RawApiEvent], config: &crate::filter::ReplayConfig) -> Result<GameResult> {
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
        for orig_evt in &sub_events {
            // Per-sub-event filter against live state
            let filtered = config.apply(orig_evt, &r.state);
            let evt = match &filtered {
                Some(e) => e,
                None => continue,
            };

            // Detect steal-of-home rewritten to remained-at-3B by filter.
            // Flag the runner so they auto-score on the next BIP.
            if orig_evt.code == "base_running"
                && attr_str(&orig_evt.attributes, "playType") == Some("stole_base")
                && attr_usize(&orig_evt.attributes, "base") == Some(4)
                && attr_str(&evt.attributes, "playType") == Some("remained_on_last_play")
            {
                if let Some(rid) = attr_str(&evt.attributes, "runnerId") {
                    r.state.held_at_third.insert(rid.to_string());
                }
            }

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

        // Resolve any pending BIP run snapshot after all sub-events
        // (including base_running corrections) have been processed.
        r.resolve_bip_snapshot();

        if need_switch {
            r.state.do_switch();
        }
    }

    if !r.state.teams_set() {
        return Err(ReplayError::MissingTeams);
    }
    Ok(aggregate(r))
}
