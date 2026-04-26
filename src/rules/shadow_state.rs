//! Lightweight game-state mirror for event-stream rule compilers.

use std::collections::HashMap;

use serde_json::Value;

use crate::event::{attr_bool, attr_str, attr_usize, PlayResult};
use crate::lineup::current_index_after_squash;

pub(super) struct ShadowState {
    bases: [Option<String>; 3],
    outs: i32,
    balls: i32,
    strikes: i32,
    need_switch: bool,
    away_id: String,
    home_id: String,
    offense: String,
    lineup: HashMap<(String, usize), String>,
    lineup_size: HashMap<String, usize>,
    current_index: HashMap<String, usize>,
}

impl ShadowState {
    pub(super) fn new() -> Self {
        Self {
            bases: [None, None, None],
            outs: 0,
            balls: 0,
            strikes: 0,
            need_switch: false,
            away_id: String::new(),
            home_id: String::new(),
            offense: String::new(),
            lineup: HashMap::new(),
            lineup_size: HashMap::new(),
            current_index: HashMap::new(),
        }
    }

    pub(super) fn is_occupied(&self, base: usize) -> bool {
        (1..=3).contains(&base) && self.bases[base - 1].is_some()
    }

    pub(super) fn remove_runner(&mut self, runner_id: &str) {
        for base in 0..3 {
            if self.bases[base].as_deref() == Some(runner_id) {
                self.bases[base] = None;
                break;
            }
        }
    }

    pub(super) fn advance_runner(&mut self, runner_id: &str, base: usize) {
        self.remove_runner(runner_id);
        if (1..=3).contains(&base) {
            self.set(base, Some(runner_id.to_string()));
        }
    }

    pub(super) fn record_runner_out(&mut self, runner_id: Option<&str>) {
        if let Some(runner_id) = runner_id {
            self.remove_runner(runner_id);
        }
        self.outs += 1;
        self.mark_switch_if_inning_over();
    }

    pub(super) fn observe_set_teams(&mut self, attrs: &Value) {
        if let (Some(away), Some(home)) = (attr_str(attrs, "awayId"), attr_str(attrs, "homeId")) {
            self.away_id = away.to_string();
            self.home_id = home.to_string();
            self.offense = away.to_string();
        }
    }

    pub(super) fn observe_fill_lineup_index(&mut self, attrs: &Value) {
        if let (Some(team_id), Some(player_id), Some(index)) = (
            attr_str(attrs, "teamId"),
            attr_str(attrs, "playerId"),
            attr_usize(attrs, "index"),
        ) {
            self.lineup
                .insert((team_id.to_string(), index), player_id.to_string());
            let size = self.lineup_size.entry(team_id.to_string()).or_insert(0);
            if index + 1 > *size {
                *size = index + 1;
            }
        }
    }

    pub(super) fn observe_fill_lineup(&mut self, attrs: &Value) {
        if let (Some(team_id), Some(player_id)) =
            (attr_str(attrs, "teamId"), attr_str(attrs, "playerId"))
        {
            let next_index = self.lineup_size.get(team_id).copied().unwrap_or(0);
            self.lineup
                .insert((team_id.to_string(), next_index), player_id.to_string());
            *self.lineup_size.entry(team_id.to_string()).or_insert(0) = next_index + 1;
        }
    }

    pub(super) fn observe_goto_lineup_index(&mut self, attrs: &Value) {
        if let (Some(team_id), Some(index)) =
            (attr_str(attrs, "teamId"), attr_usize(attrs, "index"))
        {
            self.current_index.insert(team_id.to_string(), index);
        }
    }

    pub(super) fn observe_squash_lineup_index(&mut self, attrs: &Value) {
        if let (Some(team_id), Some(index)) =
            (attr_str(attrs, "teamId"), attr_usize(attrs, "index"))
        {
            self.squash_lineup_index(team_id, index);
        }
    }

    pub(super) fn observe_reorder_lineup(&mut self, attrs: &Value) {
        if let (Some(team_id), Some(from_index), Some(to_index)) = (
            attr_str(attrs, "teamId"),
            attr_usize(attrs, "fromIndex"),
            attr_usize(attrs, "toIndex"),
        ) {
            self.reorder_lineup(team_id, from_index, to_index);
        }
    }

    pub(super) fn observe_sub_players(&mut self, attrs: &Value) {
        if let (Some(team_id), Some(outgoing_id), Some(incoming_id)) = (
            attr_str(attrs, "teamId"),
            attr_str(attrs, "outgoingPlayerId"),
            attr_str(attrs, "incomingPlayerId"),
        ) {
            self.substitute_lineup_player(team_id, outgoing_id, incoming_id);
            if attr_bool(attrs, "applyToBaserunners", false) {
                self.substitute_runner(outgoing_id, incoming_id);
            }
        }
    }

    pub(super) fn observe_pitch(&mut self, attrs: &Value) {
        if !attr_bool(attrs, "advancesCount", true) {
            return;
        }

        let result = attr_str(attrs, "result").unwrap_or("");
        match result {
            "ball" => {
                self.balls += 1;
                if self.balls >= 4 {
                    self.apply_walk();
                    self.reset_count();
                }
            }
            "strike_swinging" | "strike_looking" => {
                self.strikes += 1;
                if self.strikes >= 3 {
                    self.outs += 1;
                    self.reset_count();
                    self.advance_batter();
                    self.mark_switch_if_inning_over();
                }
            }
            "foul" if self.strikes < 2 => {
                self.strikes += 1;
            }
            "hit_by_pitch" => {
                self.apply_walk();
                self.reset_count();
            }
            "ball_in_play" => {
                self.reset_count();
            }
            _ => {}
        }
    }

    pub(super) fn observe_ball_in_play(&mut self, attrs: &Value) {
        let play_result = attr_str(attrs, "playResult").unwrap_or("");
        if PlayResult::parse(play_result).is_batter_out() {
            let added_outs = if play_result == "double_play" { 2 } else { 1 };
            self.outs += added_outs;
            self.advance_batter();
            self.mark_switch_if_inning_over();
        } else {
            self.apply_hit(play_result);
        }
    }

    pub(super) fn switch_half(&mut self) {
        self.bases = [None, None, None];
        self.outs = 0;
        self.balls = 0;
        self.strikes = 0;
        self.need_switch = false;
        self.offense = if self.offense == self.away_id {
            self.home_id.clone()
        } else {
            self.away_id.clone()
        };
    }

    pub(super) fn finish_raw_event(&mut self) -> bool {
        if self.need_switch {
            self.switch_half();
            return true;
        }
        false
    }

    fn current_batter(&self) -> Option<String> {
        let index = self.current_index.get(&self.offense).copied().unwrap_or(0);
        self.lineup.get(&(self.offense.clone(), index)).cloned()
    }

    fn advance_batter(&mut self) {
        let size = self
            .lineup_size
            .get(&self.offense)
            .copied()
            .unwrap_or(1)
            .max(1);
        let index = self.current_index.entry(self.offense.clone()).or_insert(0);
        *index = (*index + 1) % size;
    }

    fn squash_lineup_index(&mut self, team_id: &str, removed_index: usize) {
        let size = self.lineup_size.get(team_id).copied().unwrap_or(0);
        if removed_index >= size {
            return;
        }

        let team = team_id.to_string();
        self.lineup.remove(&(team.clone(), removed_index));
        for index in (removed_index + 1)..size {
            if let Some(player_id) = self.lineup.remove(&(team.clone(), index)) {
                self.lineup.insert((team.clone(), index - 1), player_id);
            }
        }

        let new_size = size - 1;
        self.lineup_size.insert(team.clone(), new_size);
        if let Some(current_index) = self.current_index.get_mut(team_id) {
            *current_index = current_index_after_squash(*current_index, size, removed_index);
        }
    }

    fn reorder_lineup(&mut self, team_id: &str, from_index: usize, to_index: usize) {
        if from_index == to_index {
            return;
        }

        let team = team_id.to_string();
        let moving = self.lineup.remove(&(team.clone(), from_index));
        if from_index < to_index {
            for index in from_index..to_index {
                if let Some(player_id) = self.lineup.remove(&(team.clone(), index + 1)) {
                    self.lineup.insert((team.clone(), index), player_id);
                }
            }
        } else {
            for index in (to_index..from_index).rev() {
                if let Some(player_id) = self.lineup.remove(&(team.clone(), index)) {
                    self.lineup.insert((team.clone(), index + 1), player_id);
                }
            }
        }

        if let Some(player_id) = moving {
            self.lineup.insert((team, to_index), player_id);
        }
    }

    fn substitute_lineup_player(&mut self, team_id: &str, outgoing_id: &str, incoming_id: &str) {
        for ((lineup_team, _), player_id) in &mut self.lineup {
            if lineup_team == team_id && player_id == outgoing_id {
                *player_id = incoming_id.to_string();
            }
        }
    }

    fn substitute_runner(&mut self, outgoing_id: &str, incoming_id: &str) {
        for runner in &mut self.bases {
            if runner.as_deref() == Some(outgoing_id) {
                *runner = Some(incoming_id.to_string());
            }
        }
    }

    fn set(&mut self, base: usize, player_id: Option<String>) {
        if (1..=3).contains(&base) {
            self.bases[base - 1] = player_id;
        }
    }

    fn reset_count(&mut self) {
        self.balls = 0;
        self.strikes = 0;
    }

    fn mark_switch_if_inning_over(&mut self) {
        if self.outs >= 3 {
            self.need_switch = true;
        }
    }

    fn apply_walk(&mut self) {
        let batter = self.current_batter();
        if self.is_occupied(1) && self.is_occupied(2) && self.is_occupied(3) {
            self.set(3, None);
        }
        if self.is_occupied(1) && self.is_occupied(2) {
            let runner_2b = self.bases[1].take();
            self.set(3, runner_2b);
        }
        if self.is_occupied(1) {
            let runner_1b = self.bases[0].take();
            self.set(2, runner_1b);
        }
        self.set(1, batter);
        self.advance_batter();
    }

    fn apply_hit(&mut self, play_result: &str) {
        let batter = self.current_batter();
        match play_result {
            "single" => {
                self.set(3, None);
                let runner_2b = self.bases[1].take();
                self.set(3, runner_2b);
                let runner_1b = self.bases[0].take();
                self.set(2, runner_1b);
                self.set(1, batter);
            }
            "double" => {
                self.set(3, None);
                self.set(2, None);
                let runner_1b = self.bases[0].take();
                self.set(3, runner_1b);
                self.set(2, batter);
            }
            "triple" | "home_run" => {
                self.bases = [None, None, None];
            }
            _ => {}
        }
        self.advance_batter();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_teams() -> Value {
        serde_json::json!({"awayId": "away", "homeId": "home"})
    }

    fn fill_lineup(team_id: &str, player_id: &str, index: usize) -> Value {
        serde_json::json!({
            "teamId": team_id,
            "playerId": player_id,
            "index": index
        })
    }

    fn pitch(result: &str) -> Value {
        serde_json::json!({"result": result, "advancesCount": true})
    }

    fn ball_in_play(play_result: &str) -> Value {
        serde_json::json!({"playResult": play_result})
    }

    fn reorder_lineup(from_index: usize, to_index: usize) -> Value {
        serde_json::json!({
            "teamId": "away",
            "fromIndex": from_index,
            "toIndex": to_index
        })
    }

    fn squash_lineup(index: usize) -> Value {
        serde_json::json!({
            "teamId": "away",
            "index": index
        })
    }

    fn sub_players(outgoing_id: &str, incoming_id: &str, apply_to_baserunners: bool) -> Value {
        serde_json::json!({
            "teamId": "away",
            "outgoingPlayerId": outgoing_id,
            "incomingPlayerId": incoming_id,
            "applyToBaserunners": apply_to_baserunners
        })
    }

    fn state_with_away_lineup() -> ShadowState {
        let mut state = ShadowState::new();
        state.observe_set_teams(&set_teams());
        state.observe_fill_lineup_index(&fill_lineup("away", "batter-1", 0));
        state.observe_fill_lineup_index(&fill_lineup("away", "batter-2", 1));
        state
    }

    #[test]
    fn walk_forces_runner_and_rotates_lineup() {
        let mut state = state_with_away_lineup();
        state.advance_runner("runner-1", 1);
        state.advance_runner("runner-2", 2);
        state.advance_runner("runner-3", 3);

        for _ in 0..4 {
            state.observe_pitch(&pitch("ball"));
        }

        assert_eq!(state.bases[0].as_deref(), Some("batter-1"));
        assert_eq!(state.bases[1].as_deref(), Some("runner-1"));
        assert_eq!(state.bases[2].as_deref(), Some("runner-2"));
        assert_eq!(state.current_batter().as_deref(), Some("batter-2"));
    }

    #[test]
    fn single_advances_runners_and_batter() {
        let mut state = state_with_away_lineup();
        state.advance_runner("runner-1", 1);
        state.advance_runner("runner-2", 2);

        state.observe_ball_in_play(&ball_in_play("single"));

        assert_eq!(state.bases[0].as_deref(), Some("batter-1"));
        assert_eq!(state.bases[1].as_deref(), Some("runner-1"));
        assert_eq!(state.bases[2].as_deref(), Some("runner-2"));
        assert_eq!(state.current_batter().as_deref(), Some("batter-2"));
    }

    #[test]
    fn strikeout_marks_switch_after_third_out() {
        let mut state = state_with_away_lineup();
        state.outs = 2;

        for _ in 0..3 {
            state.observe_pitch(&pitch("strike_swinging"));
        }

        assert!(state.need_switch);
        assert!(state.finish_raw_event());
        assert_eq!(state.outs, 0);
        assert_eq!(state.offense, "home");
    }

    #[test]
    fn runner_out_removes_runner_and_can_end_half() {
        let mut state = state_with_away_lineup();
        state.outs = 2;
        state.advance_runner("runner-1", 2);

        state.record_runner_out(Some("runner-1"));

        assert!(!state.is_occupied(2));
        assert!(state.finish_raw_event());
        assert_eq!(state.offense, "home");
    }

    #[test]
    fn reorder_lineup_changes_current_batter() {
        let mut state = state_with_away_lineup();

        state.observe_reorder_lineup(&reorder_lineup(1, 0));
        state.observe_ball_in_play(&ball_in_play("single"));

        assert_eq!(state.bases[0].as_deref(), Some("batter-2"));
        assert_eq!(state.current_batter().as_deref(), Some("batter-1"));
    }

    #[test]
    fn squash_lineup_compacts_order() {
        let mut state = state_with_away_lineup();
        state.observe_fill_lineup_index(&fill_lineup("away", "batter-3", 2));
        state.observe_goto_lineup_index(&serde_json::json!({"teamId": "away", "index": 1}));

        state.observe_squash_lineup_index(&squash_lineup(1));

        assert_eq!(state.lineup_size["away"], 2);
        assert_eq!(state.current_batter().as_deref(), Some("batter-3"));
    }

    #[test]
    fn squash_lineup_keeps_shifted_last_batter_due() {
        let mut state = state_with_away_lineup();
        state.observe_fill_lineup_index(&fill_lineup("away", "batter-3", 2));
        state.observe_fill_lineup_index(&fill_lineup("away", "batter-4", 3));
        state.observe_fill_lineup_index(&fill_lineup("away", "batter-5", 4));
        state.observe_goto_lineup_index(&serde_json::json!({"teamId": "away", "index": 4}));

        state.observe_squash_lineup_index(&squash_lineup(2));

        assert_eq!(state.lineup_size["away"], 4);
        assert_eq!(state.current_batter().as_deref(), Some("batter-5"));
    }

    #[test]
    fn sub_players_replaces_lineup_and_baserunner() {
        let mut state = state_with_away_lineup();
        state.advance_runner("batter-1", 2);

        state.observe_sub_players(&sub_players("batter-1", "pinch-runner", true));

        assert_eq!(state.current_batter().as_deref(), Some("pinch-runner"));
        assert_eq!(state.bases[1].as_deref(), Some("pinch-runner"));
    }
}
