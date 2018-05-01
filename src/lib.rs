extern crate chrono;

use chrono::prelude::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_event() {
        let event = Event{
            time: Utc::now(),
            description: "This is a description".to_string(),
            tag_ids: vec!(),
        };

        let mut event_db: EventDb = EventDb{
            events: vec!(),
        };
        event_db.events.push(event.clone());

        assert_eq!(event, event_db.events[0]);
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Event {
    time: DateTime<Utc>,
    description: String,
    tag_ids: Vec<usize>,
}

#[derive(Debug)]
struct EventDb {
    events: Vec<Event>,
}