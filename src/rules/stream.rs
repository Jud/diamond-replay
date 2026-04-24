//! Shared event-stream traversal for rule compilers.

use serde_json::Value;

use crate::error::Result;
use crate::event::RawApiEvent;

pub(super) trait EventStreamRule {
    fn apply_sub_event(&mut self, sub: Value) -> Result<Option<Value>>;

    fn finish_raw_event(&mut self) -> Result<()> {
        Ok(())
    }
}

pub(super) fn compile<R: EventStreamRule>(
    resolved: Vec<RawApiEvent>,
    rule: &mut R,
) -> Result<Vec<RawApiEvent>> {
    let mut output = Vec::with_capacity(resolved.len());

    for raw in resolved {
        let mut event_data: Value = serde_json::from_str(&raw.event_data)?;
        let is_transaction = event_data.get("events").is_some();
        let sub_events = if is_transaction {
            event_data
                .get_mut("events")
                .and_then(Value::as_array_mut)
                .map(std::mem::take)
                .unwrap_or_default()
        } else {
            vec![event_data.clone()]
        };

        let mut new_sub_events = Vec::with_capacity(sub_events.len());
        for sub in sub_events {
            if let Some(new_sub) = rule.apply_sub_event(sub)? {
                new_sub_events.push(new_sub);
            }
        }

        rule.finish_raw_event()?;

        let new_event_data = if is_transaction {
            if let Some(obj) = event_data.as_object_mut() {
                obj.insert("events".into(), Value::Array(new_sub_events));
            }
            serde_json::to_string(&event_data)?
        } else if let Some(single) = new_sub_events.into_iter().next() {
            serde_json::to_string(&single)?
        } else {
            continue;
        };

        output.push(RawApiEvent {
            id: raw.id,
            stream_id: raw.stream_id,
            sequence_number: raw.sequence_number,
            event_data: new_event_data,
        });
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DropCodeRule {
        code: &'static str,
        finished: usize,
    }

    impl DropCodeRule {
        const fn keep_all() -> Self {
            Self {
                code: "__never__",
                finished: 0,
            }
        }

        const fn drop_code(code: &'static str) -> Self {
            Self { code, finished: 0 }
        }
    }

    impl EventStreamRule for DropCodeRule {
        fn apply_sub_event(&mut self, sub: Value) -> Result<Option<Value>> {
            let code = sub.get("code").and_then(Value::as_str);
            Ok((code != Some(self.code)).then_some(sub))
        }

        fn finish_raw_event(&mut self) -> Result<()> {
            self.finished += 1;
            Ok(())
        }
    }

    fn raw(seq: i64, event_data: &Value) -> RawApiEvent {
        RawApiEvent {
            id: format!("event-{seq}"),
            stream_id: "test".to_string(),
            sequence_number: seq,
            event_data: event_data.to_string(),
        }
    }

    fn sub(code: &str) -> Value {
        serde_json::json!({"code": code, "attributes": {}})
    }

    fn transaction(events: &[Value]) -> Value {
        serde_json::json!({"code": "transaction", "events": events})
    }

    #[test]
    fn keep_all_preserves_single_events() {
        let input = vec![raw(1, &sub("pitch"))];
        let mut rule = DropCodeRule::keep_all();

        let output = compile(input.clone(), &mut rule).unwrap();

        assert_eq!(output.len(), 1);
        assert_eq!(output[0].event_data, input[0].event_data);
        assert_eq!(rule.finished, 1);
    }

    #[test]
    fn dropped_single_event_is_removed() {
        let input = vec![raw(1, &sub("base_running"))];
        let mut rule = DropCodeRule::drop_code("base_running");

        let output = compile(input, &mut rule).unwrap();

        assert!(output.is_empty());
        assert_eq!(rule.finished, 1);
    }

    #[test]
    fn dropped_transaction_sub_event_is_removed() {
        let input = vec![raw(1, &transaction(&[sub("pitch"), sub("base_running")]))];
        let mut rule = DropCodeRule::drop_code("base_running");

        let output = compile(input, &mut rule).unwrap();
        let event_data: Value = serde_json::from_str(&output[0].event_data).unwrap();
        let events = event_data["events"].as_array().unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["code"], "pitch");
        assert_eq!(rule.finished, 1);
    }

    #[test]
    fn empty_transaction_is_preserved() {
        let input = vec![raw(
            1,
            &transaction(&[sub("base_running"), sub("base_running")]),
        )];
        let mut rule = DropCodeRule::drop_code("base_running");

        let output = compile(input, &mut rule).unwrap();
        let event_data: Value = serde_json::from_str(&output[0].event_data).unwrap();

        assert!(event_data["events"].as_array().unwrap().is_empty());
        assert_eq!(rule.finished, 1);
    }
}
