use serde_json::Value;

use crate::event::RawApiEvent;

/// Pre-process undo-resolved events, applying scorebook edit operations.
///
/// `GameChanger` records post-hoc scorer corrections as append-only edit
/// events rather than rewriting stream history:
///
/// - `{"code":"edit_group","events":[{"code":"insert","beforeId":ID,
///   "events":[...]}]}` splices new events into the logical sequence
///   immediately before the existing payload event with id `ID`.
/// - `{"code":"delete","deleteIds":[ID, ...]}` removes existing payload
///   events by id.
///
/// Ids reference the parsed `event_data` payload ids (the payload object's
/// own `id`, or a sub-event's `id` inside a transaction payload) — never the
/// outer `RawApiEvent` row id.
///
/// Unknown edit operations and unresolvable targets are skipped with a
/// warning on stderr: replaying an insert at an invented position could
/// fabricate a pitcher stint, so a skipped operation (today's behavior for
/// every edit) is the safer degradation.
pub fn resolve_edits(events: Vec<RawApiEvent>) -> Vec<RawApiEvent> {
    let mut out: Vec<Entry> = Vec::with_capacity(events.len());

    for raw in events {
        let Ok(payload) = serde_json::from_str::<Value>(&raw.event_data) else {
            // Invalid payload JSON: pass through untouched so replay_game
            // reports the parse error exactly as it does today.
            out.push(Entry::passthrough(raw));
            continue;
        };
        match payload.get("code").and_then(Value::as_str) {
            Some("edit_group") => apply_edit_group(&mut out, &payload, &raw),
            Some("delete") => apply_delete(&mut out, &payload),
            _ => out.push(Entry::parsed(raw, payload)),
        }
    }

    out.into_iter().map(Entry::finalize).collect()
}

struct Entry {
    raw: RawApiEvent,
    /// Parsed `event_data`; `Value::Null` for passthrough entries whose
    /// payload failed to parse (never matched by id lookups).
    payload: Value,
    /// Whether `payload` was mutated and must be re-serialized.
    dirty: bool,
}

impl Entry {
    fn passthrough(raw: RawApiEvent) -> Self {
        Self {
            raw,
            payload: Value::Null,
            dirty: false,
        }
    }

    fn parsed(raw: RawApiEvent, payload: Value) -> Self {
        Self {
            raw,
            payload,
            dirty: false,
        }
    }

    /// Build a synthetic entry for an inserted payload event.
    fn synthetic(template: &RawApiEvent, payload: Value) -> Self {
        let id = payload
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or(&template.id)
            .to_string();
        let event_data = payload.to_string();
        Self {
            raw: RawApiEvent {
                id,
                stream_id: template.stream_id.clone(),
                sequence_number: template.sequence_number,
                event_data,
            },
            payload,
            dirty: false,
        }
    }

    fn finalize(self) -> RawApiEvent {
        let mut raw = self.raw;
        if self.dirty {
            raw.event_data = self.payload.to_string();
        }
        raw
    }

    fn payload_id(&self) -> Option<&str> {
        self.payload.get("id").and_then(Value::as_str)
    }

    /// Position of `id` among this entry's sub-events, if any.
    fn sub_event_position(&self, id: &str) -> Option<usize> {
        self.payload
            .get("events")
            .and_then(Value::as_array)?
            .iter()
            .position(|e| e.get("id").and_then(Value::as_str) == Some(id))
    }
}

/// Where an edit target id was found in the output list.
enum Target {
    /// Index of the entry whose payload id matches.
    TopLevel(usize),
    /// (entry index, sub-event index) inside a transaction payload.
    Nested(usize, usize),
}

fn find_target(out: &[Entry], id: &str) -> Option<Target> {
    for (i, entry) in out.iter().enumerate() {
        if entry.payload_id() == Some(id) {
            return Some(Target::TopLevel(i));
        }
        if let Some(j) = entry.sub_event_position(id) {
            return Some(Target::Nested(i, j));
        }
    }
    None
}

fn apply_edit_group(out: &mut Vec<Entry>, payload: &Value, raw: &RawApiEvent) {
    let Some(ops) = payload.get("events").and_then(Value::as_array) else {
        eprintln!("diamond-replay: edit_group without operations; skipped");
        return;
    };
    for op in ops {
        match op.get("code").and_then(Value::as_str) {
            Some("insert") => apply_insert(out, op, raw),
            Some("delete") => {
                apply_delete(out, op);
            }
            other => {
                eprintln!(
                    "diamond-replay: unknown edit_group operation {:?}; skipped",
                    other.unwrap_or("<missing code>")
                );
            }
        }
    }
}

fn apply_insert(out: &mut Vec<Entry>, op: &Value, raw: &RawApiEvent) {
    let Some(before_id) = op.get("beforeId").and_then(Value::as_str) else {
        eprintln!("diamond-replay: edit insert without beforeId; op skipped");
        return;
    };
    let inserted: Vec<Value> = op
        .get("events")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if inserted.is_empty() {
        return;
    }
    match find_target(out, before_id) {
        Some(Target::TopLevel(i)) => {
            // Synthetic entries take the TARGET row's stream_id and
            // sequence_number: they stand at the target's position in the
            // resolved order. (raw supplies only the fallback id.)
            let template = RawApiEvent {
                id: raw.id.clone(),
                stream_id: out[i].raw.stream_id.clone(),
                sequence_number: out[i].raw.sequence_number,
                event_data: String::new(),
            };
            for (k, payload) in inserted.into_iter().enumerate() {
                out.insert(i + k, Entry::synthetic(&template, payload));
            }
        }
        Some(Target::Nested(i, j)) => {
            let entry = &mut out[i];
            if let Some(events) = entry
                .payload
                .get_mut("events")
                .and_then(Value::as_array_mut)
            {
                for (k, payload) in inserted.into_iter().enumerate() {
                    events.insert(j + k, payload);
                }
                entry.dirty = true;
            }
        }
        None => {
            eprintln!("diamond-replay: edit insert target {before_id} not found; op skipped");
        }
    }
}

fn apply_delete(out: &mut Vec<Entry>, payload: &Value) {
    let Some(ids) = payload.get("deleteIds").and_then(Value::as_array) else {
        eprintln!("diamond-replay: delete without deleteIds; skipped");
        return;
    };
    for id in ids {
        let Some(id) = id.as_str() else { continue };
        match find_target(out, id) {
            Some(Target::TopLevel(i)) => {
                out.remove(i);
            }
            Some(Target::Nested(i, j)) => {
                let entry = &mut out[i];
                let remaining = if let Some(events) = entry
                    .payload
                    .get_mut("events")
                    .and_then(Value::as_array_mut)
                {
                    events.remove(j);
                    events.len()
                } else {
                    usize::MAX
                };
                if remaining == 0 {
                    // A transaction with no remaining sub-events carries
                    // nothing; drop the whole entry.
                    out.remove(i);
                } else {
                    out[i].dirty = true;
                }
            }
            None => {
                eprintln!("diamond-replay: edit delete target {id} not found; skipped");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Outer row ids deliberately differ from inner payload ids: edit
    /// targets reference payload ids, never row ids.
    fn raw(seq: i64, payload: &Value) -> RawApiEvent {
        RawApiEvent {
            id: format!("row-{seq}"),
            stream_id: "test".into(),
            sequence_number: seq,
            event_data: payload.to_string(),
        }
    }

    fn single(seq: i64, id: &str, code: &str) -> RawApiEvent {
        raw(seq, &serde_json::json!({"id": id, "code": code}))
    }

    fn transaction(seq: i64, id: &str, subs: &[(&str, &str)]) -> RawApiEvent {
        let events: Vec<Value> = subs
            .iter()
            .map(|(sid, code)| serde_json::json!({"id": sid, "code": code}))
            .collect();
        raw(
            seq,
            &serde_json::json!({"id": id, "code": "transaction", "events": events}),
        )
    }

    fn edit_insert(seq: i64, before_id: &str, subs: &[(&str, &str)]) -> RawApiEvent {
        let events: Vec<Value> = subs
            .iter()
            .map(|(sid, code)| serde_json::json!({"id": sid, "code": code}))
            .collect();
        raw(
            seq,
            &serde_json::json!({
                "id": format!("eg-{seq}"),
                "code": "edit_group",
                "events": [{"id": format!("ins-{seq}"), "code": "insert",
                            "beforeId": before_id, "events": events}],
            }),
        )
    }

    fn delete(seq: i64, ids: &[&str]) -> RawApiEvent {
        raw(
            seq,
            &serde_json::json!({"id": format!("del-{seq}"), "code": "delete", "deleteIds": ids}),
        )
    }

    fn codes(resolved: &[RawApiEvent]) -> Vec<String> {
        resolved
            .iter()
            .map(|r| {
                let v: Value = serde_json::from_str(&r.event_data).unwrap();
                let code = v.get("code").and_then(Value::as_str).unwrap().to_string();
                if code == "transaction" {
                    let subs: Vec<&str> = v["events"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|e| e.get("code").and_then(Value::as_str).unwrap())
                        .collect();
                    format!("transaction[{}]", subs.join(","))
                } else {
                    code
                }
            })
            .collect()
    }

    #[test]
    fn insert_before_top_level_target() {
        let events = vec![
            single(1, "a", "pitch"),
            single(2, "b", "pitch"),
            edit_insert(3, "b", &[("n1", "fill_position")]),
        ];
        let resolved = resolve_edits(events);
        assert_eq!(codes(&resolved), ["pitch", "fill_position", "pitch"]);
        assert_eq!(resolved[1].id, "n1");
    }

    #[test]
    fn insert_before_nested_target_splices_into_transaction() {
        // Defensive: unobserved in real books (all observed inserts target
        // top-level payloads), but the grammar allows it.
        let events = vec![
            transaction(1, "t1", &[("s1", "pitch"), ("s2", "base_running")]),
            edit_insert(2, "s2", &[("n1", "fill_position")]),
        ];
        let resolved = resolve_edits(events);
        assert_eq!(
            codes(&resolved),
            ["transaction[pitch,fill_position,base_running]"]
        );
    }

    #[test]
    fn two_inserts_distinct_targets_apply_in_stream_order() {
        let events = vec![
            single(1, "a", "pitch"),
            single(2, "b", "pitch"),
            edit_insert(3, "a", &[("n1", "fill_position")]),
            edit_insert(4, "b", &[("n2", "fill_position")]),
        ];
        let resolved = resolve_edits(events);
        assert_eq!(
            codes(&resolved),
            ["fill_position", "pitch", "fill_position", "pitch"]
        );
        assert_eq!(resolved[0].id, "n1");
        assert_eq!(resolved[2].id, "n2");
    }

    #[test]
    fn delete_top_level_entry() {
        let events = vec![
            single(1, "a", "pitch"),
            single(2, "b", "pitcher_decision"),
            delete(3, &["b"]),
        ];
        let resolved = resolve_edits(events);
        assert_eq!(codes(&resolved), ["pitch"]);
    }

    #[test]
    fn delete_nested_sub_event() {
        // The observed Giants shape: delete targets a pitcher_decision
        // sub-event nested inside an ordinary transaction.
        let events = vec![
            transaction(1, "t1", &[("s1", "pitch"), ("s2", "pitcher_decision")]),
            delete(2, &["s2"]),
        ];
        let resolved = resolve_edits(events);
        assert_eq!(codes(&resolved), ["transaction[pitch]"]);
    }

    #[test]
    fn delete_last_sub_event_drops_transaction() {
        let events = vec![
            transaction(1, "t1", &[("s1", "pitcher_decision")]),
            single(2, "a", "pitch"),
            delete(3, &["s1"]),
        ];
        let resolved = resolve_edits(events);
        assert_eq!(codes(&resolved), ["pitch"]);
    }

    #[test]
    fn delete_of_previously_inserted_synthetic_event() {
        // Defensive: unobserved in real books.
        let events = vec![
            single(1, "a", "pitch"),
            edit_insert(2, "a", &[("n1", "pitcher_decision")]),
            delete(3, &["n1"]),
        ];
        let resolved = resolve_edits(events);
        assert_eq!(codes(&resolved), ["pitch"]);
    }

    #[test]
    fn missing_insert_target_skips_op() {
        let events = vec![
            single(1, "a", "pitch"),
            edit_insert(2, "nope", &[("n1", "fill_position")]),
        ];
        let resolved = resolve_edits(events);
        assert_eq!(codes(&resolved), ["pitch"]);
    }

    #[test]
    fn unknown_op_code_skipped() {
        let events = vec![
            single(1, "a", "pitch"),
            raw(
                2,
                &serde_json::json!({"id": "eg", "code": "edit_group",
                    "events": [{"id": "x", "code": "replace", "targetId": "a"}]}),
            ),
        ];
        let resolved = resolve_edits(events);
        assert_eq!(codes(&resolved), ["pitch"]);
    }

    #[test]
    fn edit_group_followed_by_plain_events_keeps_order() {
        let events = vec![
            single(1, "a", "pitch"),
            edit_insert(2, "a", &[("n1", "fill_position")]),
            single(3, "b", "pitch"),
        ];
        let resolved = resolve_edits(events);
        assert_eq!(codes(&resolved), ["fill_position", "pitch", "pitch"]);
    }

    #[test]
    fn row_ids_never_match_edit_targets() {
        // Payload id namespace only: targeting an outer row id must miss.
        let events = vec![
            single(1, "a", "pitch"), // row id is "row-1"
            edit_insert(2, "row-1", &[("n1", "fill_position")]),
        ];
        let resolved = resolve_edits(events);
        assert_eq!(codes(&resolved), ["pitch"]);
    }

    // -- Cross-pass composition: resolve_undos → resolve_edits ------------

    fn undo(seq: i64) -> RawApiEvent {
        raw(seq, &serde_json::json!({"code": "undo"}))
    }

    fn redo(seq: i64) -> RawApiEvent {
        raw(seq, &serde_json::json!({"code": "redo"}))
    }

    fn resolve_both(events: Vec<RawApiEvent>) -> Vec<RawApiEvent> {
        resolve_edits(crate::undo::resolve_undos(events))
    }

    #[test]
    fn undo_removes_preceding_edit_group() {
        let events = vec![
            single(1, "a", "pitch"),
            edit_insert(2, "a", &[("n1", "fill_position")]),
            undo(3),
        ];
        let resolved = resolve_both(events);
        assert_eq!(codes(&resolved), ["pitch"]);
    }

    #[test]
    fn undo_redo_applies_edit_exactly_once() {
        let events = vec![
            single(1, "a", "pitch"),
            edit_insert(2, "a", &[("n1", "fill_position")]),
            undo(3),
            redo(4),
        ];
        let resolved = resolve_both(events);
        assert_eq!(codes(&resolved), ["fill_position", "pitch"]);
    }

    #[test]
    fn undo_removes_preceding_delete() {
        let events = vec![
            single(1, "a", "pitch"),
            single(2, "b", "pitcher_decision"),
            delete(3, &["b"]),
            undo(4),
        ];
        let resolved = resolve_both(events);
        assert_eq!(codes(&resolved), ["pitch", "pitcher_decision"]);
    }

    #[test]
    fn new_event_clears_redo_history_for_edits() {
        let events = vec![
            single(1, "a", "pitch"),
            edit_insert(2, "a", &[("n1", "fill_position")]),
            undo(3),
            single(4, "b", "pitch"),
            redo(5),
        ];
        let resolved = resolve_both(events);
        assert_eq!(codes(&resolved), ["pitch", "pitch"]);
    }

    #[test]
    fn invalid_payload_passes_through() {
        let mut bad = single(1, "a", "pitch");
        bad.event_data = "not json".into();
        let resolved = resolve_edits(vec![bad]);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].event_data, "not json");
    }
}
