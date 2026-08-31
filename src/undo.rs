use serde_json::Value;

use crate::event::RawApiEvent;

/// Pre-process raw events, removing any event that gets undone.
///
/// An `undo` event reverses the most recently applied (non-undo) event.
/// Walks the sequence-ordered stream with a stack: non-undo events are
/// pushed; each `undo` pops the top entry. Returns surviving events in
/// sequence order.
///
/// The first replacement event after an undo run inherits the undone plays'
/// game-time. A scorer who reopens the book hours after a game, undoes a
/// play, and re-enters it produces a replacement stamped with the editor's
/// wall clock — which would otherwise inflate the game's duration and pace
/// windows. The undone events carry the play's true game-time, so the first
/// event entered after the run is shifted back onto the run's earliest
/// timestamp (intra-event deltas preserved; never shifted forward). The
/// inheritance is deliberately consumed by that first event alone: how many
/// re-entered rows correspond to the undone rows is not knowable from the
/// stream, and carrying leftover timestamps forward would corrupt later,
/// genuine events. Live undo-and-rescore shifts by mere seconds; events with
/// no preceding undo (rain delays, resumed suspensions) are never touched.
///
/// Accepted limitations (all unobserved in real books beyond the seconds
/// scale): a multi-row re-entry repairs only its first row; a pure deletion
/// (undo with no re-entry) lets the next genuine event inherit the deleted
/// play's game-time slot; a pure-append correction with no undo keeps the
/// editor's wall clock.
pub fn resolve_undos(mut raw_events: Vec<RawApiEvent>) -> Vec<RawApiEvent> {
    raw_events.sort_by_key(|r| r.sequence_number);
    let mut stack: Vec<RawApiEvent> = Vec::with_capacity(raw_events.len());
    let mut undone_events: Vec<RawApiEvent> = Vec::new();

    for raw in raw_events {
        let code = extract_code(&raw.event_data);
        match code.as_deref() {
            Some("undo") => {
                if let Some(undone) = stack.pop() {
                    undone_events.push(undone);
                }
            }
            Some("redo") => {
                if let Some(restored) = undone_events.pop() {
                    stack.push(restored);
                }
            }
            _ => {
                // A new event replaces the undone run: the earliest game-time
                // across the whole run is the inheritance target, consumed by
                // this event only. Clearing also drops the redo history, as
                // before.
                let inherited = undone_events.iter().filter_map(earliest_created_at).min();
                undone_events.clear();
                let raw = match inherited {
                    Some(ts) => inherit_created_at(raw, ts),
                    None => raw,
                };
                stack.push(raw);
            }
        }
    }

    stack
}

/// Extract the top-level "code" field from `event_data` JSON without full parsing.
fn extract_code(event_data: &str) -> Option<String> {
    if event_data.contains("\"undo\"") || event_data.contains("\"redo\"") {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(event_data) {
            return v.get("code").and_then(|c| c.as_str()).map(String::from);
        }
    }
    None
}

/// Earliest `createdAt` anywhere in the event payload (top level or nested
/// sub-events), or `None` for payloads that carry no timestamp.
fn earliest_created_at(raw: &RawApiEvent) -> Option<i64> {
    let payload: Value = serde_json::from_str(&raw.event_data).ok()?;
    min_created_at(&payload)
}

fn min_created_at(value: &Value) -> Option<i64> {
    let mut min = value.get("createdAt").and_then(Value::as_i64);
    if let Some(events) = value.get("events").and_then(Value::as_array) {
        for sub in events {
            if let Some(sub_min) = min_created_at(sub) {
                min = Some(match min {
                    Some(current) => current.min(sub_min),
                    None => sub_min,
                });
            }
        }
    }
    min
}

/// Shift every `createdAt` in the payload so its earliest timestamp lands on
/// `target_ts`, preserving intra-event deltas. Events whose own timestamps
/// are not later than the target are left untouched (an inheritance must
/// only ever pull a replacement BACK toward the play it corrects). An event
/// whose timestamps cannot be shifted without overflow is left unchanged.
fn inherit_created_at(raw: RawApiEvent, target_ts: i64) -> RawApiEvent {
    let Ok(mut payload) = serde_json::from_str::<Value>(&raw.event_data) else {
        return raw;
    };
    let Some(own_min) = min_created_at(&payload) else {
        return raw;
    };
    if own_min <= target_ts {
        return raw;
    }
    let Some(delta) = own_min.checked_sub(target_ts) else {
        return raw;
    };
    if !shift_is_safe(&payload, delta) {
        return raw;
    }
    shift_created_at(&mut payload, delta);
    RawApiEvent {
        event_data: payload.to_string(),
        ..raw
    }
}

fn shift_is_safe(value: &Value, delta: i64) -> bool {
    if let Some(ts) = value.get("createdAt").and_then(Value::as_i64) {
        if ts.checked_sub(delta).is_none() {
            return false;
        }
    }
    if let Some(events) = value.get("events").and_then(Value::as_array) {
        return events.iter().all(|sub| shift_is_safe(sub, delta));
    }
    true
}

fn shift_created_at(value: &mut Value, delta: i64) {
    if let Some(ts) = value.get("createdAt").and_then(Value::as_i64) {
        if let Some(shifted) = ts.checked_sub(delta) {
            value["createdAt"] = Value::from(shifted);
        }
    }
    if let Some(events) = value.get_mut("events").and_then(Value::as_array_mut) {
        for sub in events {
            shift_created_at(sub, delta);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(seq: i64, code: &str) -> RawApiEvent {
        RawApiEvent {
            id: format!("evt-{seq}"),
            stream_id: "test".into(),
            sequence_number: seq,
            event_data: format!(r#"{{"code":"{code}"}}"#),
        }
    }

    fn make_ts_event(seq: i64, code: &str, ts: i64) -> RawApiEvent {
        RawApiEvent {
            id: format!("evt-{seq}"),
            stream_id: "test".into(),
            sequence_number: seq,
            event_data: format!(r#"{{"code":"{code}","createdAt":{ts}}}"#),
        }
    }

    fn created_at_of(raw: &RawApiEvent) -> Option<i64> {
        earliest_created_at(raw)
    }

    #[test]
    fn test_simple_undo() {
        let events = vec![
            make_event(1, "pitch"),
            make_event(2, "pitch"),
            make_event(3, "undo"),
        ];
        let resolved = resolve_undos(events);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].sequence_number, 1);
    }

    #[test]
    fn test_double_undo() {
        let events = vec![
            make_event(1, "pitch"),
            make_event(2, "pitch"),
            make_event(3, "undo"),
            make_event(4, "undo"),
        ];
        let resolved = resolve_undos(events);
        assert!(resolved.is_empty());
    }

    #[test]
    fn test_undo_empty_stack() {
        let events = vec![make_event(1, "undo")];
        let resolved = resolve_undos(events);
        assert!(resolved.is_empty());
    }

    #[test]
    fn test_no_undos() {
        let events = vec![make_event(1, "pitch"), make_event(2, "ball_in_play")];
        let resolved = resolve_undos(events);
        assert_eq!(resolved.len(), 2);
    }

    #[test]
    fn test_out_of_order_input() {
        let events = vec![
            make_event(3, "undo"),
            make_event(1, "pitch"),
            make_event(2, "pitch"),
        ];
        let resolved = resolve_undos(events);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].sequence_number, 1);
    }

    // -- Replacement timestamp inheritance --------------------------------

    #[test]
    fn late_rescore_inherits_undone_plays_timestamp() {
        // The McCabe shape: play at game time, undone, re-entered 3h later.
        let game_ts = 1_000_000_000_000;
        let editor_ts = game_ts + 3 * 60 * 60 * 1000;
        let events = vec![
            make_ts_event(1, "pitch", game_ts),
            make_event(2, "undo"),
            make_ts_event(3, "pitch", editor_ts),
        ];
        let resolved = resolve_undos(events);
        assert_eq!(resolved.len(), 1);
        assert_eq!(created_at_of(&resolved[0]), Some(game_ts));
    }

    #[test]
    fn first_replacement_inherits_burst_earliest_time_only() {
        // n undone, m re-entered: only the FIRST re-entry inherits (the
        // pairing between the rest is not knowable from the stream), from
        // the earliest time across the whole undone run.
        let events = vec![
            make_ts_event(1, "pitch", 100),
            make_ts_event(2, "base_running", 200),
            make_event(3, "undo"),
            make_event(4, "undo"),
            make_ts_event(5, "pitch", 90_000),
            make_ts_event(6, "base_running", 90_500),
        ];
        let resolved = resolve_undos(events);
        assert_eq!(resolved.len(), 2);
        assert_eq!(created_at_of(&resolved[0]), Some(100));
        assert_eq!(created_at_of(&resolved[1]), Some(90_500));
    }

    #[test]
    fn later_genuine_events_never_consume_stale_inheritance() {
        // n > m: two events undone, one re-entered, then real play continues.
        // The continuation must keep its own timestamp — leftover undone
        // times must not leak forward.
        let events = vec![
            make_ts_event(1, "pitch", 2_000),
            make_ts_event(2, "base_running", 3_000),
            make_event(3, "undo"),
            make_event(4, "undo"),
            make_ts_event(5, "pitch", 90_000),
            make_ts_event(6, "pitch", 200_000),
        ];
        let resolved = resolve_undos(events);
        assert_eq!(created_at_of(&resolved[0]), Some(2_000));
        assert_eq!(created_at_of(&resolved[1]), Some(200_000));
    }

    #[test]
    fn undone_event_without_timestamp_still_yields_burst_minimum() {
        let events = vec![
            make_ts_event(1, "pitch", 5_000),
            make_event(2, "fill_position"), // no createdAt
            make_event(3, "undo"),
            make_event(4, "undo"),
            make_ts_event(5, "pitch", 700_000),
        ];
        let resolved = resolve_undos(events);
        assert_eq!(created_at_of(&resolved[0]), Some(5_000));
    }

    #[test]
    fn timestampless_replacement_consumes_and_drops_inheritance() {
        // The first re-entry has no timestamp to repair; the inheritance is
        // consumed regardless so it cannot leak onto later genuine events.
        let events = vec![
            make_ts_event(1, "pitch", 5_000),
            make_event(2, "undo"),
            make_event(3, "fill_position"), // no createdAt
            make_ts_event(4, "pitch", 700_000),
        ];
        let resolved = resolve_undos(events);
        assert_eq!(created_at_of(&resolved[0]), None);
        assert_eq!(created_at_of(&resolved[1]), Some(700_000));
    }

    #[test]
    fn overflow_unsafe_shift_leaves_replacement_unchanged() {
        let events = vec![
            make_ts_event(1, "pitch", i64::MIN),
            make_event(2, "undo"),
            make_ts_event(3, "pitch", i64::MAX),
        ];
        let resolved = resolve_undos(events);
        assert_eq!(created_at_of(&resolved[0]), Some(i64::MAX));
    }

    #[test]
    fn undo_of_shifted_replacement_then_redo_keeps_inherited_time() {
        let events = vec![
            make_ts_event(1, "pitch", 1_000),
            make_event(2, "undo"),
            make_ts_event(3, "pitch", 500_000), // inherits 1_000
            make_event(4, "undo"),
            make_event(5, "redo"),
        ];
        let resolved = resolve_undos(events);
        assert_eq!(resolved.len(), 1);
        assert_eq!(created_at_of(&resolved[0]), Some(1_000));
    }

    #[test]
    fn replacement_earlier_than_undone_play_keeps_own_timestamp() {
        let events = vec![
            make_ts_event(1, "pitch", 5_000),
            make_event(2, "undo"),
            make_ts_event(3, "pitch", 4_000),
        ];
        let resolved = resolve_undos(events);
        assert_eq!(created_at_of(&resolved[0]), Some(4_000));
    }

    #[test]
    fn transaction_replacement_shifts_nested_timestamps_preserving_deltas() {
        let undone_play = RawApiEvent {
            id: "v".into(),
            stream_id: "test".into(),
            sequence_number: 1,
            event_data: r#"{"code":"transaction","events":[{"code":"pitch","createdAt":1000},{"code":"ball_in_play","createdAt":1200}]}"#.into(),
        };
        let replacement = RawApiEvent {
            id: "r".into(),
            stream_id: "test".into(),
            sequence_number: 3,
            event_data: r#"{"code":"transaction","events":[{"code":"pitch","createdAt":700000},{"code":"ball_in_play","createdAt":700300}]}"#.into(),
        };
        let resolved = resolve_undos(vec![undone_play, make_event(2, "undo"), replacement]);
        assert_eq!(resolved.len(), 1);
        let payload: Value = serde_json::from_str(&resolved[0].event_data).unwrap();
        let subs = payload["events"].as_array().unwrap();
        assert_eq!(subs[0]["createdAt"], 1000);
        assert_eq!(subs[1]["createdAt"], 1300); // 300ms delta preserved
    }

    #[test]
    fn events_without_preceding_undo_are_never_shifted() {
        // A resumed suspended game: a huge forward jump with no undo stays.
        let events = vec![
            make_ts_event(1, "pitch", 1_000),
            make_ts_event(2, "pitch", 1_000_000_000),
        ];
        let resolved = resolve_undos(events);
        assert_eq!(created_at_of(&resolved[1]), Some(1_000_000_000));
    }

    #[test]
    fn redo_restores_undone_event_before_inheritance_harvest() {
        // undo, redo (restores), then a NEW event: nothing stays undone, so the
        // new event keeps its own timestamp.
        let events = vec![
            make_ts_event(1, "pitch", 1_000),
            make_event(2, "undo"),
            make_event(3, "redo"),
            make_ts_event(4, "pitch", 999_000),
        ];
        let resolved = resolve_undos(events);
        assert_eq!(resolved.len(), 2);
        assert_eq!(created_at_of(&resolved[1]), Some(999_000));
    }
}
