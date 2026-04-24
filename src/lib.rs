#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod compute;
mod error;
mod event;
mod options;
mod player;
mod replay;
mod rules;
mod score;
pub mod stat_help;
mod state;
mod undo;

pub use error::{ReplayError, Result};
pub use event::RawApiEvent;
pub use options::{ReplayOptions, RuleSet};
pub use player::{BattingStats, PitchingStats, PlayerGameStats};
pub use replay::{GameResult, LittleLeagueStats};

/// Replay a complete game from raw scoring event stream data.
///
/// # Errors
///
/// Returns an error if the event list is empty, teams are not set,
/// or any event data fails to parse as JSON.
pub fn replay(raw_events: Vec<RawApiEvent>) -> Result<GameResult> {
    replay_with_options(raw_events, ReplayOptions::standard())
}

/// Replay a complete game from raw scoring event stream data with options.
///
/// # Errors
///
/// Returns an error if the event list is empty, teams are not set,
/// or any event data fails to parse as JSON.
pub fn replay_with_options(
    raw_events: Vec<RawApiEvent>,
    options: ReplayOptions,
) -> Result<GameResult> {
    let resolved = undo::resolve_undos(raw_events);
    let compiled = rules::compile_events(resolved, options.rule_set)?;
    replay::replay_game(&compiled)
}

/// Convenience: parse a JSON array of raw API events and replay.
///
/// # Errors
///
/// Returns an error if the JSON is invalid, event list is empty,
/// teams are not set, or any event data fails to parse.
pub fn replay_from_json(json: &str) -> Result<GameResult> {
    replay_from_json_with_options(json, ReplayOptions::standard())
}

/// Convenience: parse a JSON array of raw API events and replay with options.
///
/// # Errors
///
/// Returns an error if the JSON is invalid, event list is empty,
/// teams are not set, or any event data fails to parse.
pub fn replay_from_json_with_options(json: &str, options: ReplayOptions) -> Result<GameResult> {
    let raw_events: Vec<RawApiEvent> = serde_json::from_str(json)?;
    replay_with_options(raw_events, options)
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
    replay_from_json_with_options(json, ReplayOptions::no_steal_home())
}
