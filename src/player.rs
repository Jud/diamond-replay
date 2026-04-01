use std::collections::HashMap;

use crate::event::{BrPlayType, PlayResult};

// ---------------------------------------------------------------------------
// Per-player stat structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct BattingStats {
    pub pa: i32,
    pub k: i32,
    pub k_looking: i32,
    pub k_swinging: i32,
    pub bb: i32,
    pub hbp: i32,
    pub singles: i32,
    pub doubles: i32,
    pub triples: i32,
    pub home_runs: i32,
    pub sac_fly: i32,
    pub sac_bunt: i32,
    pub fc: i32,
    pub roe: i32,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct BaserunningStats {
    pub runs: i32,
    pub sb: i32,
    pub cs: i32,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct PitchingStats {
    pub pitches: i32,
    pub balls: i32,
    pub strikes: i32,
    pub k: i32,
    pub bb: i32,
    pub hbp: i32,
    pub hits_allowed: i32,
    pub hr_allowed: i32,
    pub runs_allowed: i32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlayerGameStats {
    pub player_id: String,
    pub team_id: String,
    pub batting: BattingStats,
    pub baserunning: BaserunningStats,
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
        }
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
                baserunning: BaserunningStats::default(),
                pitching: None,
            })
    }

    // -- Batting stat recording ---------------------------------------------

    /// Record a strikeout for the current batter.
    pub fn record_k(&mut self, team_id: &str, looking: bool) {
        if let Some(pid) = self.current_batter(team_id).map(str::to_string) {
            let s = &mut self.ensure_stats(&pid).batting;
            s.pa += 1;
            s.k += 1;
            if looking {
                s.k_looking += 1;
            } else {
                s.k_swinging += 1;
            }
        }
        self.advance_batter(team_id);
    }

    /// Record a walk for the current batter.
    pub fn record_bb(&mut self, team_id: &str) {
        if let Some(pid) = self.current_batter(team_id).map(str::to_string) {
            let s = &mut self.ensure_stats(&pid).batting;
            s.pa += 1;
            s.bb += 1;
        }
        self.advance_batter(team_id);
    }

    /// Record an HBP for the current batter.
    pub fn record_hbp(&mut self, team_id: &str) {
        if let Some(pid) = self.current_batter(team_id).map(str::to_string) {
            let s = &mut self.ensure_stats(&pid).batting;
            s.pa += 1;
            s.hbp += 1;
        }
        self.advance_batter(team_id);
    }

    /// Record a ball-in-play result for the current batter.
    pub fn record_bip(&mut self, team_id: &str, play_result: PlayResult) {
        if let Some(pid) = self.current_batter(team_id).map(str::to_string) {
            let s = &mut self.ensure_stats(&pid).batting;
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
        }
        self.advance_batter(team_id);
    }

    /// Record a dropped third strike (counts as K + PA, batter may or may not reach).
    pub fn record_dropped_k(&mut self, team_id: &str, looking: bool) {
        if let Some(pid) = self.current_batter(team_id).map(str::to_string) {
            let s = &mut self.ensure_stats(&pid).batting;
            s.pa += 1;
            s.k += 1;
            if looking {
                s.k_looking += 1;
            } else {
                s.k_swinging += 1;
            }
        }
        self.advance_batter(team_id);
    }

    // -- Baserunning stat recording -----------------------------------------

    pub fn record_run(&mut self, runner_id: &str) {
        self.ensure_stats(runner_id).baserunning.runs += 1;
    }

    /// Undo a previously recorded run for a runner.
    pub fn undo_run(&mut self, runner_id: &str) {
        self.ensure_stats(runner_id).baserunning.runs -= 1;
    }

    /// Undo a previously recorded run allowed for the defense team's pitcher.
    pub fn undo_pitch_run(&mut self, defense_team: &str) {
        if let Some(pid) = self.current_pitcher(defense_team).map(str::to_string) {
            if let Some(ref mut p) = self.ensure_stats(&pid).pitching {
                p.runs_allowed -= 1;
            }
        }
    }

    pub fn record_sb(&mut self, runner_id: &str) {
        self.ensure_stats(runner_id).baserunning.sb += 1;
    }

    pub fn record_cs(&mut self, runner_id: &str) {
        self.ensure_stats(runner_id).baserunning.cs += 1;
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

    // -- Pitching stat recording --------------------------------------------

    /// Record a pitch thrown by the defense team's pitcher.
    pub fn record_pitch_thrown(&mut self, defense_team: &str, is_ball: bool) {
        if let Some(pid) = self.current_pitcher(defense_team).map(str::to_string) {
            let stats = self.ensure_stats(&pid);
            let p = stats.pitching.get_or_insert_with(PitchingStats::default);
            p.pitches += 1;
            if is_ball {
                p.balls += 1;
            } else {
                p.strikes += 1;
            }
        }
    }

    /// Record a K by the pitcher.
    pub fn record_pitch_k(&mut self, defense_team: &str) {
        if let Some(pid) = self.current_pitcher(defense_team).map(str::to_string) {
            let p = self
                .ensure_stats(&pid)
                .pitching
                .get_or_insert_with(PitchingStats::default);
            p.k += 1;
        }
    }

    /// Record a walk allowed by the pitcher.
    pub fn record_pitch_bb(&mut self, defense_team: &str) {
        if let Some(pid) = self.current_pitcher(defense_team).map(str::to_string) {
            let p = self
                .ensure_stats(&pid)
                .pitching
                .get_or_insert_with(PitchingStats::default);
            p.bb += 1;
        }
    }

    /// Record an HBP by the pitcher.
    pub fn record_pitch_hbp(&mut self, defense_team: &str) {
        if let Some(pid) = self.current_pitcher(defense_team).map(str::to_string) {
            let p = self
                .ensure_stats(&pid)
                .pitching
                .get_or_insert_with(PitchingStats::default);
            p.hbp += 1;
        }
    }

    /// Record a hit allowed by the pitcher.
    pub fn record_pitch_hit(&mut self, defense_team: &str, play_result: PlayResult) {
        if let Some(pid) = self.current_pitcher(defense_team).map(str::to_string) {
            let p = self
                .ensure_stats(&pid)
                .pitching
                .get_or_insert_with(PitchingStats::default);
            p.hits_allowed += 1;
            if play_result == PlayResult::HomeRun {
                p.hr_allowed += 1;
            }
        }
    }

    /// Record a run allowed by the pitcher.
    pub fn record_pitch_run(&mut self, defense_team: &str) {
        if let Some(pid) = self.current_pitcher(defense_team).map(str::to_string) {
            let p = self
                .ensure_stats(&pid)
                .pitching
                .get_or_insert_with(PitchingStats::default);
            p.runs_allowed += 1;
        }
    }

    /// Remove runs from players on a team when a score override reduces the total.
    /// Removes from the last players first (reverse insertion order approximation).
    ///
    /// # Panics
    ///
    /// Panics if a player ID found in iteration is missing from the stats map
    /// (should never happen since the IDs are drawn from the same map).
    pub fn adjust_team_runs(&mut self, team_id: &str, delta: i32) {
        if delta >= 0 {
            return;
        }
        let mut remaining = -delta;
        let mut ids: Vec<String> = self
            .stats
            .iter()
            .filter(|(_, s)| s.team_id == team_id && s.baserunning.runs > 0)
            .map(|(id, _)| id.clone())
            .collect();
        ids.reverse();
        for id in ids {
            if remaining == 0 {
                break;
            }
            let runs = &mut self.stats.get_mut(&id).unwrap().baserunning.runs;
            let take = (*runs).min(remaining);
            *runs -= take;
            remaining -= take;
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
