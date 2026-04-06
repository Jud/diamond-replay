#![allow(clippy::module_name_repetitions)]

use crate::event::{attr_str, attr_usize, SubEvent};
use crate::state::GameState;

/// Decides what to do with a single sub-event during dispatch.
/// Runs per-sub-event inside the dispatch loop, seeing live `GameState`
/// after prior sub-events in the same transaction have already mutated it.
pub enum FilterAction {
    /// Process this event normally.
    Keep,
    /// Replace this event with a modified version.
    Replace(SubEvent),
    /// Drop this event entirely (skip dispatch).
    Drop,
}

/// Transforms sub-events one at a time against live game state.
pub trait EventFilter {
    fn filter_event(&self, event: &SubEvent, state: &GameState) -> FilterAction;
}

/// Configuration for replay variants.
pub struct ReplayConfig {
    pub filters: Vec<Box<dyn EventFilter>>,
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self { filters: vec![] }
    }
}

impl ReplayConfig {
    /// Apply all filters to a single sub-event. Returns `None` if any filter drops it.
    pub fn apply(&self, event: &SubEvent, state: &GameState) -> Option<SubEvent> {
        let mut current = event.clone();
        for f in &self.filters {
            match f.filter_event(&current, state) {
                FilterAction::Keep => {}
                FilterAction::Replace(replaced) => current = replaced,
                FilterAction::Drop => return None,
            }
        }
        Some(current)
    }
}

/// Suppress all "chaos" scoring from 3B: steals, wild pitches, passed balls.
/// Runners can only score on hits, walks, and HBP (i.e., from the BIP/walk
/// auto-advance path). Confirmation events for already-scored runners are dropped.
pub struct NoStealHomeFilter;

/// Play types that represent chaos scoring from 3B (not hits or walks).
const CHAOS_PLAY_TYPES: &[&str] = &[
    "stole_base",
    "wild_pitch",
    "passed_ball",
];

impl EventFilter for NoStealHomeFilter {
    fn filter_event(&self, event: &SubEvent, state: &GameState) -> FilterAction {
        if event.code != "base_running" {
            return FilterAction::Keep;
        }
        let pt = attr_str(&event.attributes, "playType").unwrap_or("");
        let base = attr_usize(&event.attributes, "base");
        if base != Some(4) || !CHAOS_PLAY_TYPES.contains(&pt) {
            return FilterAction::Keep;
        }

        // Check live GameState: is the runner actually on a base?
        // Because this runs per-sub-event INSIDE the dispatch loop,
        // state.bases reflects mutations from earlier sub-events in
        // the same transaction (e.g., auto-advance from a BIP).
        let runner_id = attr_str(&event.attributes, "runnerId");
        let on_bases = runner_id
            .is_some_and(|rid| state.bases.find_by_id(rid).is_some());

        if on_bases {
            // Runner on bases: rewrite to stay at 3B.
            let mut new_evt = event.clone();
            let mut attrs = new_evt
                .attributes
                .as_object()
                .cloned()
                .unwrap_or_default();
            attrs.insert(
                "playType".into(),
                serde_json::Value::String("remained_on_last_play".into()),
            );
            attrs.insert("base".into(), serde_json::Value::Number(3.into()));
            new_evt.attributes = serde_json::Value::Object(attrs);
            FilterAction::Replace(new_evt)
        } else {
            // Runner not on bases: confirmation of already-scored runner.
            // Drop it.
            FilterAction::Drop
        }
    }
}
