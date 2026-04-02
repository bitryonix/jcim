use std::collections::VecDeque;

use crate::model::EventLine;

/// Fixed number of recent events retained per bounded in-memory event queue.
const EVENT_LIMIT: usize = 32;

/// Append one event and evict the oldest retained items once the queue reaches capacity.
pub(super) fn remember_event(
    queue: &mut VecDeque<EventLine>,
    level: &str,
    message: impl Into<String>,
) {
    queue.push_back(EventLine {
        level: level.to_string(),
        message: message.into(),
    });
    while queue.len() > EVENT_LIMIT {
        queue.pop_front();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remember_event_preserves_order_and_truncates_oldest_entries() {
        let mut queue = VecDeque::new();
        for index in 0..(EVENT_LIMIT + 3) {
            remember_event(&mut queue, "info", format!("event-{index}"));
        }

        assert_eq!(queue.len(), EVENT_LIMIT);
        assert_eq!(queue.front().expect("first event").message, "event-3");
        assert_eq!(queue.back().expect("last event").message, "event-34");
        assert!(queue.iter().all(|event| event.level == "info"));
    }
}
