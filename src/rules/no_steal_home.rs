//! No-steal-home rule compiler.
//!
//! Transforms an undo-resolved event stream before feeding it to the
//! core replay engine. The core engine stays pure with zero rule hooks.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::stream::{self, EventStreamRule};
use crate::error::Result;
use crate::event::{attr_bool, attr_str, attr_usize, BrPlayType, PlayResult, RawApiEvent};

/// Lightweight base/out tracker for the scenario compiler.
///
/// ```text
/// Decision logic for chaos base_running events:
///
///   base=4 (home)?  → ALWAYS block. Runner stays at 3rd.
///   base=2 or 3?    → Block if destination is occupied (no room).
///                      Allow if destination is empty.
///   base=1?         → Allow (steals to 1st don't happen, but be safe).
///
/// Everything else (hits, walks, normal advances) passes through
/// unchanged. The core engine's auto-advance and walk force-advance
/// score runs naturally when bases load up.
///
/// Scorer corrections for diverged runners (whose chaos advance was
/// blocked) are dropped so they don't undo the rule's auto-advance.
/// ```
struct NoStealHomeState {
    bases: [Option<String>; 3], // 0=1B, 1=2B, 2=3B
    outs: i32,
    balls: i32,
    strikes: i32,
    need_switch: bool,
    diverged: HashSet<String>,
    // Lineup tracking for batter ID resolution
    away_id: String,
    home_id: String,
    offense: String,
    lineup: HashMap<(String, usize), String>, // (team, slot) → player ID
    lineup_size: HashMap<String, usize>,
    current_index: HashMap<String, usize>,
}

impl NoStealHomeState {
    fn new() -> Self {
        Self {
            bases: [None, None, None],
            outs: 0,
            balls: 0,
            strikes: 0,
            need_switch: false,
            diverged: HashSet::new(),
            away_id: String::new(),
            home_id: String::new(),
            offense: String::new(),
            lineup: HashMap::new(),
            lineup_size: HashMap::new(),
            current_index: HashMap::new(),
        }
    }

    fn current_batter(&self) -> Option<String> {
        let idx = self.current_index.get(&self.offense).copied().unwrap_or(0);
        self.lineup.get(&(self.offense.clone(), idx)).cloned()
    }

    fn advance_batter(&mut self) {
        let size = self
            .lineup_size
            .get(&self.offense)
            .copied()
            .unwrap_or(1)
            .max(1);
        let idx = self.current_index.entry(self.offense.clone()).or_insert(0);
        *idx = (*idx + 1) % size;
    }

    fn is_occupied(&self, base: usize) -> bool {
        (1..=3).contains(&base) && self.bases[base - 1].is_some()
    }

    fn set(&mut self, base: usize, id: Option<String>) {
        if (1..=3).contains(&base) {
            self.bases[base - 1] = id;
        }
    }

    fn remove_runner(&mut self, rid: &str) {
        for b in 0..3 {
            if self.bases[b].as_deref() == Some(rid) {
                self.bases[b] = None;
                break;
            }
        }
    }

    fn switch_half(&mut self) {
        self.bases = [None, None, None];
        self.outs = 0;
        self.balls = 0;
        self.strikes = 0;
        self.diverged.clear();
        self.offense = if self.offense == self.away_id {
            self.home_id.clone()
        } else {
            self.away_id.clone()
        };
    }

    fn reset_count(&mut self) {
        self.balls = 0;
        self.strikes = 0;
    }

    /// Walk/HBP force-advance: push runners up, place batter on 1B.
    fn apply_walk(&mut self) {
        let batter = self.current_batter();
        if self.is_occupied(1) && self.is_occupied(2) && self.is_occupied(3) {
            self.set(3, None); // runner scores
        }
        if self.is_occupied(1) && self.is_occupied(2) {
            let r2 = self.bases[1].take();
            self.set(3, r2);
        }
        if self.is_occupied(1) {
            let r1 = self.bases[0].take();
            self.set(2, r1);
        }
        self.set(1, batter);
        self.advance_batter();
    }

    /// Auto-advance for hits. Places batter on the appropriate base.
    fn apply_hit(&mut self, play_result: &str) {
        let batter = self.current_batter();
        match play_result {
            "single" => {
                self.set(3, None); // 3B scores
                let r2 = self.bases[1].take();
                self.set(3, r2); // 2B→3B
                let r1 = self.bases[0].take();
                self.set(2, r1); // 1B→2B
                self.set(1, batter);
            }
            "double" => {
                self.set(3, None); // 3B scores
                self.set(2, None); // 2B scores
                let r1 = self.bases[0].take();
                self.set(3, r1); // 1B→3B
                self.set(2, batter);
            }
            "triple" | "home_run" => {
                self.bases = [None, None, None];
            }
            _ => {} // corrections follow via base_running
        }
        self.advance_batter();
    }

    fn handle_set_teams(&mut self, attrs: &Value) {
        if let (Some(away), Some(home)) = (attr_str(attrs, "awayId"), attr_str(attrs, "homeId")) {
            self.away_id = away.to_string();
            self.home_id = home.to_string();
            self.offense = away.to_string();
        }
    }

    fn handle_fill_lineup_index(&mut self, attrs: &Value) {
        if let (Some(tid), Some(pid), Some(idx)) = (
            attr_str(attrs, "teamId"),
            attr_str(attrs, "playerId"),
            attr_usize(attrs, "index"),
        ) {
            self.lineup.insert((tid.to_string(), idx), pid.to_string());
            let size = self.lineup_size.entry(tid.to_string()).or_insert(0);
            if idx + 1 > *size {
                *size = idx + 1;
            }
        }
    }

    fn handle_fill_lineup(&mut self, attrs: &Value) {
        if let (Some(tid), Some(pid)) = (attr_str(attrs, "teamId"), attr_str(attrs, "playerId")) {
            let next_idx = self.lineup_size.get(tid).copied().unwrap_or(0);
            self.lineup
                .insert((tid.to_string(), next_idx), pid.to_string());
            *self.lineup_size.entry(tid.to_string()).or_insert(0) = next_idx + 1;
        }
    }

    fn handle_goto_lineup_index(&mut self, attrs: &Value) {
        if let (Some(tid), Some(idx)) = (attr_str(attrs, "teamId"), attr_usize(attrs, "index")) {
            self.current_index.insert(tid.to_string(), idx);
        }
    }

    fn handle_pitch(&mut self, attrs: &Value) {
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
                    if self.outs >= 3 {
                        self.need_switch = true;
                    }
                }
            }
            "foul" => {
                if self.strikes < 2 {
                    self.strikes += 1;
                }
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

    fn handle_ball_in_play(&mut self, attrs: &Value) {
        let pr = attr_str(attrs, "playResult").unwrap_or("");
        if PlayResult::parse(pr).is_batter_out() {
            let added = if pr == "double_play" { 2 } else { 1 };
            self.outs += added;
            self.advance_batter();
            if self.outs >= 3 {
                self.need_switch = true;
            }
        } else {
            self.apply_hit(pr);
        }
    }

    fn handle_base_running(&mut self, attrs: &Value) -> bool {
        let pt = attr_str(attrs, "playType").unwrap_or("");
        let br_type = BrPlayType::parse(pt);
        let base = attr_usize(attrs, "base");
        let runner_id = attr_str(attrs, "runnerId").map(str::to_string);

        if br_type.is_chaos() {
            return self.handle_chaos_base_running(base, runner_id.as_deref());
        }

        if runner_id
            .as_ref()
            .is_some_and(|rid| self.diverged.contains(rid) && !br_type.is_out())
        {
            return false;
        }

        if br_type.is_out() {
            if let Some(rid) = &runner_id {
                self.remove_runner(rid);
                self.diverged.remove(rid);
            }
            self.outs += 1;
            if self.outs >= 3 {
                self.need_switch = true;
            }
            return true;
        }

        if let (Some(rid), Some(b)) = (&runner_id, base) {
            self.remove_runner(rid);
            if (1..=3).contains(&b) {
                self.set(b, Some(rid.clone()));
            }
        }
        true
    }

    fn handle_chaos_base_running(&mut self, base: Option<usize>, runner_id: Option<&str>) -> bool {
        let blocked = match base {
            Some(4) => true,
            Some(b @ 1..=3) => self.is_occupied(b),
            _ => false,
        };

        if blocked {
            if let Some(rid) = runner_id {
                self.diverged.insert(rid.to_string());
            }
            return false;
        }

        if let (Some(rid), Some(b)) = (runner_id, base) {
            self.remove_runner(rid);
            self.set(b, Some(rid.to_string()));
        }
        true
    }

    fn handle_end_half(&mut self) {
        self.switch_half();
        self.need_switch = false;
    }
}

/// Compile the no-steal-home scenario. Chaos base-running events are
/// blocked when they would score (base=4) or when the destination base
/// is already occupied. Runners only score via hits or walk force-advance.
///
/// # Errors
///
/// Returns an error if any `event_data` fails to parse as JSON.
pub(super) fn compile(resolved: Vec<RawApiEvent>) -> Result<Vec<RawApiEvent>> {
    let mut rule = NoStealHomeState::new();
    stream::compile(resolved, &mut rule)
}

impl EventStreamRule for NoStealHomeState {
    fn apply_sub_event(&mut self, sub: Value) -> Result<Option<Value>> {
        let code = attr_str(&sub, "code").unwrap_or("");
        let null = Value::Null;
        let attrs = sub.get("attributes").unwrap_or(&null);

        let keep = match code {
            "set_teams" => {
                self.handle_set_teams(attrs);
                true
            }
            "fill_lineup_index" => {
                self.handle_fill_lineup_index(attrs);
                true
            }
            "fill_lineup" => {
                self.handle_fill_lineup(attrs);
                true
            }
            "goto_lineup_index" => {
                self.handle_goto_lineup_index(attrs);
                true
            }
            "pitch" => {
                self.handle_pitch(attrs);
                true
            }
            "ball_in_play" => {
                self.handle_ball_in_play(attrs);
                true
            }
            "base_running" => self.handle_base_running(attrs),
            "end_half" => {
                self.handle_end_half();
                true
            }
            _ => true,
        };

        Ok(keep.then_some(sub))
    }

    fn finish_raw_event(&mut self) -> Result<()> {
        if self.need_switch {
            self.switch_half();
            self.need_switch = false;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::needless_pass_by_value)]
    fn raw_single(seq: i64, sub: serde_json::Value) -> RawApiEvent {
        RawApiEvent {
            id: format!("test-{seq}"),
            stream_id: "test".into(),
            sequence_number: seq,
            event_data: serde_json::to_string(&sub).unwrap(),
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    fn raw_tx(seq: i64, subs: Vec<serde_json::Value>) -> RawApiEvent {
        let tx = serde_json::json!({"code": "transaction", "events": subs});
        RawApiEvent {
            id: format!("test-{seq}"),
            stream_id: "test".into(),
            sequence_number: seq,
            event_data: serde_json::to_string(&tx).unwrap(),
        }
    }

    fn pitch(result: &str) -> serde_json::Value {
        serde_json::json!({"code": "pitch", "attributes": {"result": result, "advancesCount": true}})
    }

    fn bip(play_result: &str) -> serde_json::Value {
        serde_json::json!({"code": "ball_in_play", "attributes": {"playResult": play_result}})
    }

    fn base_running(play_type: &str, base: u64, runner_id: &str) -> serde_json::Value {
        serde_json::json!({"code": "base_running", "attributes": {"playType": play_type, "base": base, "runnerId": runner_id}})
    }

    fn end_half() -> serde_json::Value {
        serde_json::json!({"code": "end_half", "attributes": {}})
    }

    #[test]
    fn chaos_scoring_always_blocked() {
        let events = vec![
            raw_single(1, base_running("advanced_on_last_play", 3, "r1")),
            raw_single(2, base_running("passed_ball", 4, "r1")),
        ];
        let result = compile(events).unwrap();
        assert_eq!(result.len(), 1, "PB to home dropped");
    }

    #[test]
    fn chaos_advance_allowed_when_destination_empty() {
        let events = vec![
            raw_single(1, base_running("advanced_on_last_play", 1, "r1")),
            raw_single(2, base_running("passed_ball", 2, "r1")), // 2B empty, allowed
        ];
        let result = compile(events).unwrap();
        assert_eq!(result.len(), 2, "PB to empty 2B allowed");
    }

    #[test]
    fn chaos_advance_blocked_when_destination_occupied() {
        let events = vec![
            raw_single(1, base_running("advanced_on_last_play", 2, "r1")),
            raw_single(2, base_running("advanced_on_last_play", 3, "r2")),
            // r2 on 3rd, PB to home blocked. r1 on 2nd, PB to 3rd blocked (occupied).
            raw_single(3, base_running("passed_ball", 4, "r2")), // blocked
            raw_single(4, base_running("passed_ball", 3, "r1")), // blocked (3B occupied)
        ];
        let result = compile(events).unwrap();
        assert_eq!(result.len(), 2, "both chaos advances blocked");
    }

    #[test]
    fn bases_load_and_walk_forces_run() {
        // Simulate: 3 runners get on via advances, PB scoring blocked,
        // then walks should force runs via the core engine
        let events = vec![
            raw_single(1, base_running("advanced_on_last_play", 1, "r1")),
            raw_single(2, base_running("advanced_on_last_play", 2, "r2")),
            raw_single(3, base_running("advanced_on_last_play", 3, "r3")),
            raw_single(4, base_running("stole_base", 4, "r3")), // blocked
            // r3 still on 3rd. Bases loaded (r1=1B, r2=2B, r3=3B)
            // Walk should force r3 home via core engine
            raw_single(5, pitch("ball")),
            raw_single(6, pitch("ball")),
            raw_single(7, pitch("ball")),
            raw_single(8, pitch("ball")),
        ];
        let result = compile(events).unwrap();
        // Event 4 (steal to home) should be dropped
        assert_eq!(result.len(), 7);
    }

    #[test]
    fn non_chaos_events_pass_through() {
        let events = vec![
            raw_single(1, pitch("ball")),
            raw_single(2, bip("single")),
            raw_single(3, base_running("advanced_on_last_play", 3, "r1")),
            raw_single(4, base_running("caught_stealing", 2, "r2")),
        ];
        let result = compile(events).unwrap();
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn half_inning_switch_clears_state() {
        let events = vec![
            raw_single(1, base_running("advanced_on_last_play", 3, "r1")),
            raw_single(2, end_half()),
            // After switch, 3B is empty so PB advance should be allowed
            raw_single(3, base_running("advanced_on_last_play", 2, "r2")),
            raw_single(4, base_running("passed_ball", 3, "r2")), // 3B empty, allowed
        ];
        let result = compile(events).unwrap();
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn chaos_within_transaction() {
        let events = vec![
            raw_single(1, base_running("advanced_on_last_play", 3, "r1")),
            raw_tx(
                2,
                vec![
                    base_running("passed_ball", 4, "r1"), // blocked
                    base_running("passed_ball", 3, "r2"), // blocked (3B occupied)
                ],
            ),
        ];
        let result = compile(events).unwrap();
        let ed: serde_json::Value = serde_json::from_str(&result[1].event_data).unwrap();
        let tx_events = ed["events"].as_array().unwrap();
        assert_eq!(tx_events.len(), 0, "both chaos events in tx dropped");
    }

    #[test]
    fn sacrifice_fly_counted_as_out() {
        let events = vec![raw_tx(1, vec![bip("sacrifice_fly")])];
        let result = compile(events).unwrap();
        assert_eq!(result.len(), 1);
    }
}
