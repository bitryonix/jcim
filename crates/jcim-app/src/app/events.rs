use std::collections::VecDeque;

use crate::model::EventLine;

const EVENT_LIMIT: usize = 32;

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
