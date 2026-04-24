//! Event-stream rule compilation.
//!
//! Rule compilers transform an undo-resolved event stream before the core
//! replay engine sees it. Standard replay is an identity compiler.

mod no_steal_home;
mod shadow_state;
mod standard;
mod stream;

use crate::error::Result;
use crate::event::RawApiEvent;
use crate::options::RuleSet;

pub(crate) fn compile_events(
    events: Vec<RawApiEvent>,
    rule_set: RuleSet,
) -> Result<Vec<RawApiEvent>> {
    match rule_set {
        RuleSet::Standard => Ok(standard::compile(events)),
        RuleSet::NoStealHome => no_steal_home::compile(events),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(seq: i64, code: &str) -> RawApiEvent {
        RawApiEvent {
            id: format!("event-{seq}"),
            stream_id: "test".to_string(),
            sequence_number: seq,
            event_data: serde_json::json!({"code": code, "attributes": {}}).to_string(),
        }
    }

    #[test]
    fn standard_compiler_is_identity() {
        let input = vec![raw(1, "set_teams"), raw(2, "end_half")];
        let output = compile_events(input.clone(), RuleSet::Standard).unwrap();

        assert_eq!(output.len(), input.len());
        for (actual, expected) in output.iter().zip(input) {
            assert_eq!(actual.id, expected.id);
            assert_eq!(actual.stream_id, expected.stream_id);
            assert_eq!(actual.sequence_number, expected.sequence_number);
            assert_eq!(actual.event_data, expected.event_data);
        }
    }

    #[test]
    fn no_steal_home_compiler_drops_home_chaos_advance() {
        let input = vec![RawApiEvent {
            id: "event-1".to_string(),
            stream_id: "test".to_string(),
            sequence_number: 1,
            event_data: serde_json::json!({
                "code": "base_running",
                "attributes": {
                    "playType": "stole_base",
                    "base": 4,
                    "runnerId": "runner-1"
                }
            })
            .to_string(),
        }];

        let output = compile_events(input, RuleSet::NoStealHome).unwrap();
        assert!(output.is_empty());
    }
}
