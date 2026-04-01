#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod compute;
pub mod error;
pub mod event;
pub mod player;
pub mod replay;
pub mod score;
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
