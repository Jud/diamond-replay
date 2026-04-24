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
    let mut undo_victims: Vec<RawApiEvent> = Vec::new();

    for raw in raw_events {
        let code = extract_code(&raw.event_data);
        match code.as_deref() {
            Some("undo") => {
                if let Some(victim) = stack.pop() {
                    undo_victims.push(victim);
                }
            }
            Some("redo") => {
                if let Some(restored) = undo_victims.pop() {
                    stack.push(restored);
                }
            }
            _ => {
                // Any new non-undo/redo event clears the redo history
                undo_victims.clear();
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
