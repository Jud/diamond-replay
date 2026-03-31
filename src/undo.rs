use crate::event::RawApiEvent;

/// Pre-process raw events, removing any event that gets undone.
///
/// An `undo` event reverses the most recently applied (non-undo) event.
/// Walks the sequence-ordered stream with a stack: non-undo events are
/// pushed; each `undo` pops the top entry. Returns surviving events in
/// sequence order.
pub fn resolve_undos(mut raw_events: Vec<RawApiEvent>) -> Vec<RawApiEvent> {
    raw_events.sort_by_key(|r| r.sequence_number);
    let mut stack: Vec<RawApiEvent> = Vec::with_capacity(raw_events.len());

    for raw in raw_events {
        // Quick check: does this event's data start with undo?
        // We parse just enough to detect the code.
        let is_undo = raw.event_data.contains("\"undo\"");
        if is_undo {
            // Verify it's actually the code field, not some attribute value
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw.event_data) {
                if v.get("code").and_then(|c| c.as_str()) == Some("undo") {
                    stack.pop();
                    continue;
                }
            }
        }
        stack.push(raw);
    }

    stack
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(seq: i64, code: &str) -> RawApiEvent {
        RawApiEvent {
            id: format!("evt-{seq}"),
            stream_id: "test".into(),
            sequence_number: seq,
            event_data: format!(r#"{{"code":"{}"}}"#, code),
        }
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
}
