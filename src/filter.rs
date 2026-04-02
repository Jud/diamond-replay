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

/// Suppress steals of home: runners attempting to steal home stay at 3B.
/// Confirmation events for already-scored runners are dropped.
pub struct NoStealHomeFilter;

impl EventFilter for NoStealHomeFilter {
    fn filter_event(&self, event: &SubEvent, state: &GameState) -> FilterAction {
        if event.code != "base_running" {
            return FilterAction::Keep;
        }
        let pt = attr_str(&event.attributes, "playType").unwrap_or("");
        let base = attr_usize(&event.attributes, "base");
        if pt != "stole_base" || base != Some(4) {
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
            // Real steal attempt. Rewrite: runner stays at 3B.
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
            // Drop it. No steal happened in our simulation.
            FilterAction::Drop
        }
    }
}
