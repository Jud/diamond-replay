#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod compute;
pub mod error;
pub mod event;
pub mod player;
pub mod replay;
pub mod score;
pub mod sim;
pub mod stat_help;
pub mod state;
mod undo;

use error::Result;
use event::RawApiEvent;
pub use replay::GameResult;

/// Replay a complete game from raw scoring event stream data.
///
/// # Errors
///
/// Returns an error if the event list is empty, teams are not set,
/// or any event data fails to parse as JSON.
pub fn replay(raw_events: Vec<RawApiEvent>) -> Result<GameResult> {
    let resolved = undo::resolve_undos(raw_events);
    replay::replay_game(&resolved)
}

/// Convenience: parse a JSON array of raw API events and replay.
///
/// # Errors
///
/// Returns an error if the JSON is invalid, event list is empty,
/// teams are not set, or any event data fails to parse.
pub fn replay_from_json(json: &str) -> Result<GameResult> {
    let raw_events: Vec<RawApiEvent> = serde_json::from_str(json)?;
    replay(raw_events)
}

/// Replay with no-steal-home simulation: chaos scoring from 3B
/// (steals, wild pitches, passed balls) is suppressed. Held runners
/// auto-score on the next ball in play that doesn't end the inning.
///
/// # Errors
///
/// Returns an error if the JSON is invalid, event list is empty,
/// teams are not set, or any event data fails to parse.
pub fn replay_from_json_no_steal_home(json: &str) -> Result<GameResult> {
    let raw_events: Vec<RawApiEvent> = serde_json::from_str(json)?;
    let resolved = undo::resolve_undos(raw_events);
    let compiled = sim::compile_no_steal_home(resolved)?;
    replay::replay_game(&compiled)
}
