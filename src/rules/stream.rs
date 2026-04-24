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
