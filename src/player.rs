use std::collections::HashMap;

use crate::event::{BipPlayType, BrPlayType, PitchResult, PlayResult};
use crate::state::PAContext;

// ---------------------------------------------------------------------------
// Spray chart entry — one per ball in play with location data
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct DefenderInfo {
    pub position: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub error: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SprayEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    pub result: String,
    pub bip_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hr_location: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub defenders: Vec<DefenderInfo>,
}

// ---------------------------------------------------------------------------
// Per-player stat structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct BattingStats {
    // Raw counts
    pub pa: i32,
    pub k: i32,
    pub k_looking: i32,
    pub k_swinging: i32,
    pub bb: i32,
    pub hbp: i32,
    pub ci: i32,
    pub singles: i32,
    pub doubles: i32,
    pub triples: i32,
    pub home_runs: i32,
    pub sac_fly: i32,
    pub sac_bunt: i32,
    pub fc: i32,
    pub roe: i32,
    pub gidp: i32,
    pub rbi: i32,
    pub runs: i32,
    pub ground_balls: i32,
    pub fly_balls: i32,
    pub line_drives: i32,
    pub pop_ups: i32,
    pub hard_hit_balls: i32,
    pub pitches_seen: i32,
    pub qab: i32,
    pub competitive_ab: i32,
    pub sb: i32,
    pub cs: i32,

    // Spray chart data (one entry per BIP with location)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub spray_chart: Vec<SprayEntry>,

    // Derived integer fields (computed after replay)
    pub ab: i32,
    pub hits: i32,
    pub tb: i32,
    pub xbh: i32,

    // Derived rate fields (computed after replay)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obp: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slg: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ops: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iso: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub babip: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub k_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bb_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bb_k: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub woba: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gb_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fb_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ld_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hr_fb: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p_pa: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qab_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub competitive_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hard_hit_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sb_pct: Option<f64>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct PitchingStats {
    // Raw counts
    pub pitches: i32,
    pub balls: i32,
    pub strikes_swinging: i32,
    pub strikes_looking: i32,
    pub fouls: i32,
    pub k: i32,
    pub bb: i32,
    pub hbp: i32,
    pub hits_allowed: i32,
    pub hr_allowed: i32,
    pub runs_allowed: i32,
    pub earned_runs_allowed: i32,
    pub outs_recorded: i32,
    pub bf: i32,
    pub bip: i32,
    pub ground_balls: i32,
    pub fly_balls: i32,
    pub line_drives: i32,
    pub pop_ups: i32,
    pub first_pitch_strikes: i32,
    pub wp: i32,

    // Derived rate fields (computed after replay)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub era: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whip: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub k9: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bb9: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h9: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hr9: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub k_bb: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fip: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub k_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bb_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub k_bb_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub babip: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hr_fb: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gb_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fb_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ld_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sw_str_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub csw_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c_str_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fps_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub foul_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_score: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pitches_per_ip: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlayerGameStats {
    pub player_id: String,
    pub team_id: String,
    pub batting: BattingStats,
    pub pitching: Option<PitchingStats>,
}

// ---------------------------------------------------------------------------
// Lineup + pitcher tracker
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct PlayerTracker {
    /// Maps (team, lineup slot) to player.
    lineup: HashMap<(String, usize), String>,
    /// Lineup size per team (max index + 1)
    lineup_size: HashMap<String, usize>,
    /// Current batting index per team
    current_index: HashMap<String, usize>,
    /// Current pitcher per team
    current_pitcher: HashMap<String, String>,
    /// Maps player to team.
    player_team: HashMap<String, String>,
    /// Accumulated stats per player
    stats: HashMap<String, PlayerGameStats>,
    /// Player run credits in scoring order, used for deterministic score corrections.
    run_log: Vec<String>,
}

impl PlayerTracker {
    #[must_use]
    pub fn new() -> Self {
        Self {
            lineup: HashMap::new(),
            lineup_size: HashMap::new(),
            current_index: HashMap::new(),
            current_pitcher: HashMap::new(),
            player_team: HashMap::new(),
            stats: HashMap::new(),
            run_log: Vec::new(),
        }
    }

    pub fn handle_fill_lineup(&mut self, team_id: &str, player_id: &str, index: usize) {
        self.lineup
            .insert((team_id.to_string(), index), player_id.to_string());
        let size = self.lineup_size.entry(team_id.to_string()).or_insert(0);
        if index + 1 > *size {
            *size = index + 1;
        }
        self.player_team
            .insert(player_id.to_string(), team_id.to_string());
    }

    /// Record a player from a `fill_lineup` event (no explicit index).
    /// Assigns the next sequential lineup slot for the team.
    pub fn handle_fill_lineup_roster(&mut self, team_id: &str, player_id: &str) {
        let next_idx = self.lineup_size.get(team_id).copied().unwrap_or(0);
        self.handle_fill_lineup(team_id, player_id, next_idx);
    }

    pub fn handle_fill_position(&mut self, team_id: &str, player_id: &str, position: &str) {
        self.player_team
            .insert(player_id.to_string(), team_id.to_string());
        if position == "P" {
            self.current_pitcher
                .insert(team_id.to_string(), player_id.to_string());
            // Ensure pitcher has PitchingStats initialized
            let stats = self.ensure_stats(player_id);
            if stats.pitching.is_none() {
                stats.pitching = Some(PitchingStats::default());
            }
        }
    }

    /// Remove a lineup slot and shift higher indices down by one.
    /// GC fires this after `clear_lineup_index` to compact the lineup
    /// when a player is removed mid-game.
    pub fn handle_squash_lineup(&mut self, team_id: &str, removed_index: usize) {
        let size = self.lineup_size.get(team_id).copied().unwrap_or(0);
        if removed_index >= size {
            return;
        }
        self.lineup.remove(&(team_id.to_string(), removed_index));
        for i in (removed_index + 1)..size {
            if let Some(pid) = self.lineup.remove(&(team_id.to_string(), i)) {
                self.lineup.insert((team_id.to_string(), i - 1), pid);
            }
        }
        let new_size = size - 1;
        self.lineup_size.insert(team_id.to_string(), new_size);
        if let Some(idx) = self.current_index.get_mut(team_id) {
            if new_size == 0 {
                *idx = 0;
            } else if *idx >= size {
                *idx %= new_size;
            } else if *idx > removed_index {
                *idx -= 1;
            } else if *idx >= new_size {
                *idx = 0;
            }
        }
    }

    /// Move a player from one batting order position to another, shifting
    /// intermediate positions. GC fires this when the scorer drags a player
    /// in the lineup — it's an insert, not a swap.
    pub fn handle_reorder_lineup(&mut self, team_id: &str, from_index: usize, to_index: usize) {
        if from_index == to_index {
            return;
        }
        let tid = team_id.to_string();
        let moving = self.lineup.remove(&(tid.clone(), from_index));
        if from_index < to_index {
            // Moving down: shift indices from+1..=to up by one
            for i in from_index..to_index {
                if let Some(pid) = self.lineup.remove(&(tid.clone(), i + 1)) {
                    self.lineup.insert((tid.clone(), i), pid);
                }
            }
        } else {
            // Moving up: shift indices to..from-1 down by one
            for i in (to_index..from_index).rev() {
                if let Some(pid) = self.lineup.remove(&(tid.clone(), i)) {
                    self.lineup.insert((tid.clone(), i + 1), pid);
                }
            }
        }
        if let Some(pid) = moving {
            self.lineup.insert((tid, to_index), pid);
        }
    }

    /// Substitute one player for another. GC fires `sub_players` when a
    /// coach makes a substitution. The incoming player takes the outgoing
    /// player's lineup slot, field position, and (optionally) base.
    pub fn handle_sub_players(
        &mut self,
        team_id: &str,
        outgoing_id: &str,
        incoming_id: &str,
        _apply_to_baserunners: bool,
    ) {
        // Replace in lineup map
        for (key, pid) in &mut self.lineup {
            if key.0 == team_id && pid == outgoing_id {
                *pid = incoming_id.to_string();
            }
        }
        // Transfer team membership
        self.player_team
            .insert(incoming_id.to_string(), team_id.to_string());
        // Transfer pitcher role if applicable
        if self.current_pitcher.get(team_id).map(String::as_str) == Some(outgoing_id) {
            self.current_pitcher
                .insert(team_id.to_string(), incoming_id.to_string());
            let stats = self.ensure_stats(incoming_id);
            if stats.pitching.is_none() {
                stats.pitching = Some(PitchingStats::default());
            }
        }
        // The apply_to_baserunners flag is returned for the caller to handle
        // (base state lives on Replay, not PlayerTracker).
    }

    pub fn handle_goto(&mut self, team_id: &str, index: usize) {
        self.current_index.insert(team_id.to_string(), index);
    }

    /// Get the current batter for a team. Returns None if lineup is unknown.
    #[must_use]
    pub fn current_batter(&self, team_id: &str) -> Option<&str> {
        let idx = self.current_index.get(team_id).copied().unwrap_or(0);
        self.lineup
            .get(&(team_id.to_string(), idx))
            .map(String::as_str)
    }

    /// Get the current pitcher for a team.
    #[must_use]
    pub fn current_pitcher(&self, team_id: &str) -> Option<&str> {
        self.current_pitcher.get(team_id).map(String::as_str)
    }

    /// Advance the batting order after a plate appearance completes.
    pub fn advance_batter(&mut self, team_id: &str) {
        let size = self.lineup_size.get(team_id).copied().unwrap_or(1);
        let idx = self.current_index.entry(team_id.to_string()).or_insert(0);
        *idx = (*idx + 1) % size;
    }

    /// Get or create stats for a player.
    fn ensure_stats(&mut self, player_id: &str) -> &mut PlayerGameStats {
        let team_id = self.player_team.get(player_id).cloned().unwrap_or_default();
        self.stats
            .entry(player_id.to_string())
            .or_insert_with(|| PlayerGameStats {
                player_id: player_id.to_string(),
                team_id,
                batting: BattingStats::default(),
                pitching: None,
            })
    }

    /// Get or create stats for a player with an explicit team ID.
    /// Used for anonymous fallback players whose IDs are not in `player_team`.
    fn ensure_stats_for_team(&mut self, player_id: &str, team_id: &str) -> &mut PlayerGameStats {
        self.stats
            .entry(player_id.to_string())
            .or_insert_with(|| PlayerGameStats {
                player_id: player_id.to_string(),
                team_id: team_id.to_string(),
                batting: BattingStats::default(),
                pitching: None,
            })
    }

    // -- Helper closures ------------------------------------------------------

    fn with_batter<F: FnOnce(&mut BattingStats)>(&mut self, team_id: &str, f: F) {
        let pid = self
            .current_batter(team_id)
            .map_or_else(|| format!("__anon_batter_{team_id}"), str::to_string);
        f(&mut self.ensure_stats_for_team(&pid, team_id).batting);
    }

    fn with_pitcher<F: FnOnce(&mut PitchingStats)>(&mut self, defense_team: &str, f: F) {
        let pid = self
            .current_pitcher(defense_team)
            .map_or_else(|| format!("__anon_pitcher_{defense_team}"), str::to_string);
        let stats = self.ensure_stats_for_team(&pid, defense_team);
        let p = stats.pitching.get_or_insert_with(PitchingStats::default);
        f(p);
    }

    // -- Batting stat recording -----------------------------------------------

    /// Record a strikeout for the current batter.
    pub fn record_k(&mut self, team_id: &str, looking: bool) {
        self.with_batter(team_id, |s| {
            s.pa += 1;
            s.k += 1;
            if looking {
                s.k_looking += 1;
            } else {
                s.k_swinging += 1;
            }
        });
        self.advance_batter(team_id);
    }

    /// Record a walk for the current batter.
    pub fn record_bb(&mut self, team_id: &str) {
        self.with_batter(team_id, |s| {
            s.pa += 1;
            s.bb += 1;
        });
        self.advance_batter(team_id);
    }

    /// Record an HBP for the current batter.
    pub fn record_hbp(&mut self, team_id: &str) {
        self.with_batter(team_id, |s| {
            s.pa += 1;
            s.hbp += 1;
        });
        self.advance_batter(team_id);
    }

    /// Record catcher interference for the current batter.
    pub fn record_ci(&mut self, team_id: &str) {
        self.with_batter(team_id, |s| {
            s.pa += 1;
            s.ci += 1;
        });
        self.advance_batter(team_id);
    }

    /// Record a ball-in-play result for the current batter.
    pub fn record_bip(
        &mut self,
        team_id: &str,
        play_result: PlayResult,
        bip_type: BipPlayType,
        spray: Option<SprayEntry>,
    ) {
        self.with_batter(team_id, |s| {
            s.pa += 1;
            match play_result {
                PlayResult::Single => s.singles += 1,
                PlayResult::Double => s.doubles += 1,
                PlayResult::Triple => s.triples += 1,
                PlayResult::HomeRun => s.home_runs += 1,
                PlayResult::SacrificeFly => s.sac_fly += 1,
                PlayResult::SacrificeBunt => s.sac_bunt += 1,
                PlayResult::FieldersChoice => s.fc += 1,
                PlayResult::Error => s.roe += 1,
                _ => {} // other batter-out types: no hit credit
            }
            // Record batted ball type
            match bip_type {
                BipPlayType::GroundBall => s.ground_balls += 1,
                BipPlayType::HardGroundBall => {
                    s.ground_balls += 1;
                    s.hard_hit_balls += 1;
                }
                BipPlayType::FlyBall => s.fly_balls += 1,
                BipPlayType::LineDrive => {
                    s.line_drives += 1;
                    s.hard_hit_balls += 1;
                }
                BipPlayType::PopFly => s.pop_ups += 1,
                BipPlayType::Other => {}
            }
            if let Some(entry) = spray {
                s.spray_chart.push(entry);
            }
        });
        self.advance_batter(team_id);
    }

    // -- Run / baserunning stat recording -------------------------------------

    pub fn record_run(&mut self, runner_id: &str) {
        self.ensure_stats(runner_id).batting.runs += 1;
        self.run_log.push(runner_id.to_string());
    }

    /// Undo a previously recorded run for a runner.
    pub fn undo_run(&mut self, runner_id: &str) {
        self.ensure_stats(runner_id).batting.runs -= 1;
        if let Some(pos) = self.run_log.iter().rposition(|id| id == runner_id) {
            self.run_log.remove(pos);
        }
    }

    pub fn record_sb(&mut self, runner_id: &str) {
        self.ensure_stats(runner_id).batting.sb += 1;
    }

    pub fn record_cs(&mut self, runner_id: &str) {
        self.ensure_stats(runner_id).batting.cs += 1;
    }

    pub fn record_baserunning(&mut self, runner_id: &str, play_type: BrPlayType, base: usize) {
        if play_type == BrPlayType::CaughtStealing {
            self.record_cs(runner_id);
        }
        if play_type == BrPlayType::StoleBase {
            self.record_sb(runner_id);
        }
        if base == 4 && !play_type.is_out() {
            self.record_run(runner_id);
        }
    }

    /// Record an RBI for a specific player.
    pub fn record_rbi(&mut self, batter_id: &str, count: i32) {
        self.ensure_stats(batter_id).batting.rbi += count;
    }

    /// Record a GIDP for a specific player by ID.
    pub fn record_gidp(&mut self, batter_id: &str) {
        self.ensure_stats(batter_id).batting.gidp += 1;
    }

    /// Record a quality at-bat for the current batter.
    pub fn record_qab(&mut self, team_id: &str) {
        self.with_batter(team_id, |s| {
            s.qab += 1;
        });
    }

    /// Record a competitive at-bat for the current batter.
    pub fn record_competitive_ab(&mut self, team_id: &str) {
        self.with_batter(team_id, |s| {
            s.competitive_ab += 1;
        });
    }

    /// Transfer per-PA context data for the batter and pitcher.
    pub fn record_pa_context(&mut self, team_id: &str, defense_team: &str, ctx: &PAContext) {
        // Batter: accumulate pitches seen
        let pitches = ctx.pitches_in_pa;
        self.with_batter(team_id, |s| {
            s.pitches_seen += pitches;
        });
        // Pitcher: first pitch strike
        if ctx.first_pitch_strike {
            self.with_pitcher(defense_team, |p| {
                p.first_pitch_strikes += 1;
            });
        }
    }

    // -- Pitching stat recording ----------------------------------------------

    /// Increment the appropriate batted-ball counter on `PitchingStats`.
    fn record_bip_type(p: &mut PitchingStats, bip_type: BipPlayType) {
        match bip_type {
            BipPlayType::GroundBall | BipPlayType::HardGroundBall => p.ground_balls += 1,
            BipPlayType::FlyBall => p.fly_balls += 1,
            BipPlayType::LineDrive => p.line_drives += 1,
            BipPlayType::PopFly => p.pop_ups += 1,
            BipPlayType::Other => {}
        }
    }

    /// Record a pitch thrown by the defense team's pitcher.
    pub fn record_pitch_thrown(&mut self, defense_team: &str, result: PitchResult) {
        self.with_pitcher(defense_team, |p| {
            p.pitches += 1;
            match result {
                PitchResult::Ball => p.balls += 1,
                PitchResult::StrikeSwinging => p.strikes_swinging += 1,
                PitchResult::StrikeLooking => p.strikes_looking += 1,
                PitchResult::Foul => p.fouls += 1,
                PitchResult::BallInPlay | PitchResult::HitByPitch | PitchResult::Unknown => {}
            }
        });
    }

    /// Record a K by the pitcher.
    pub fn record_pitch_k(&mut self, defense_team: &str) {
        self.with_pitcher(defense_team, |p| {
            p.k += 1;
        });
    }

    /// Record a walk allowed by the pitcher.
    pub fn record_pitch_bb(&mut self, defense_team: &str) {
        self.with_pitcher(defense_team, |p| {
            p.bb += 1;
        });
    }

    /// Record an HBP by the pitcher.
    pub fn record_pitch_hbp(&mut self, defense_team: &str) {
        self.with_pitcher(defense_team, |p| {
            p.hbp += 1;
        });
    }

    /// Record a hit allowed by the pitcher, including batted ball type.
    pub fn record_pitch_hit(
        &mut self,
        defense_team: &str,
        play_result: PlayResult,
        bip_type: BipPlayType,
    ) {
        self.with_pitcher(defense_team, |p| {
            p.hits_allowed += 1;
            if play_result == PlayResult::HomeRun {
                p.hr_allowed += 1;
            }
            p.bip += 1;
            Self::record_bip_type(p, bip_type);
        });
    }

    /// Record a batted ball in play for the pitcher (outs, FC, errors).
    pub fn record_pitch_bip(&mut self, defense_team: &str, bip_type: BipPlayType) {
        self.with_pitcher(defense_team, |p| {
            p.bip += 1;
            Self::record_bip_type(p, bip_type);
        });
    }

    /// Record a run allowed by the pitcher.
    pub fn record_pitch_run(&mut self, defense_team: &str) {
        self.with_pitcher(defense_team, |p| {
            p.runs_allowed += 1;
        });
    }

    /// Record an earned run allowed by the pitcher.
    pub fn record_pitch_earned_run(&mut self, defense_team: &str) {
        self.with_pitcher(defense_team, |p| {
            p.earned_runs_allowed += 1;
        });
    }

    /// Undo a previously recorded run allowed for the defense team's pitcher.
    pub fn undo_pitch_run(&mut self, defense_team: &str) {
        self.with_pitcher(defense_team, |p| {
            p.runs_allowed -= 1;
        });
    }

    /// Undo a previously recorded earned run for the defense team's pitcher.
    pub fn undo_pitch_earned_run(&mut self, defense_team: &str) {
        self.with_pitcher(defense_team, |p| {
            p.earned_runs_allowed -= 1;
        });
    }

    /// Record an out recorded by the pitcher.
    pub fn record_pitch_out(&mut self, defense_team: &str) {
        self.with_pitcher(defense_team, |p| {
            p.outs_recorded += 1;
        });
    }

    /// Record a batter faced by the pitcher.
    pub fn record_pitch_bf(&mut self, defense_team: &str) {
        self.with_pitcher(defense_team, |p| {
            p.bf += 1;
        });
    }

    /// Record a wild pitch by the pitcher.
    pub fn record_pitch_wp(&mut self, defense_team: &str) {
        self.with_pitcher(defense_team, |p| {
            p.wp += 1;
        });
    }

    /// Remove runs from players on a team when a score override reduces the total.
    /// Removes from the latest recorded run credits first.
    ///
    /// # Panics
    ///
    /// Panics if a player ID found in iteration is missing from the stats map
    /// (should never happen since the IDs are drawn from the run log).
    pub fn adjust_team_runs(&mut self, team_id: &str, delta: i32) {
        if delta >= 0 {
            return;
        }
        let mut remaining = -delta;
        while remaining > 0 {
            let Some(pos) = self.run_log.iter().rposition(|id| {
                self.stats
                    .get(id)
                    .is_some_and(|s| s.team_id == team_id && s.batting.runs > 0)
            }) else {
                break;
            };
            let id = self.run_log.remove(pos);
            let runs = &mut self.stats.get_mut(&id).unwrap().batting.runs;
            *runs -= 1;
            remaining -= 1;
        }
    }

    /// Adjust the current pitcher's `runs_allowed` and `earned_runs_allowed` by a delta.
    /// Used when a score override changes the run total for a team.
    pub fn adjust_pitch_runs(&mut self, defense_team: &str, delta: i32) {
        if let Some(pid) = self.current_pitcher(defense_team).map(str::to_string) {
            if let Some(ref mut p) = self.ensure_stats(&pid).pitching {
                p.runs_allowed += delta;
                p.earned_runs_allowed += delta;
            }
        }
    }

    /// Consume the tracker and return the accumulated player stats.
    #[must_use]
    pub fn into_stats(self) -> HashMap<String, PlayerGameStats> {
        self.stats
    }
}

impl Default for PlayerTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::PlayerTracker;

    #[test]
    fn squash_lineup_keeps_shifted_last_batter_due() {
        let mut players = PlayerTracker::new();
        for index in 0..5 {
            players.handle_fill_lineup("away", &format!("batter-{index}"), index);
        }
        players.handle_goto("away", 4);

        players.handle_squash_lineup("away", 2);

        assert_eq!(players.current_batter("away"), Some("batter-4"));
    }

    #[test]
    fn squash_lineup_wraps_when_due_batter_is_removed_last_slot() {
        let mut players = PlayerTracker::new();
        for index in 0..3 {
            players.handle_fill_lineup("away", &format!("batter-{index}"), index);
        }
        players.handle_goto("away", 2);

        players.handle_squash_lineup("away", 2);

        assert_eq!(players.current_batter("away"), Some("batter-0"));
    }
}
