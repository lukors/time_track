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

        let mut event_db = EventDB::new();
        event_db.events.push(event.clone());

        assert_eq!(event, event_db.events[0]);
    }

    // #[test]
    // fn write_db() {
    //     let event_db: EventDB = EventDB{
    //         events: vec!(),
    //     }
    //     let file_name = "write_test";
        
    //     super::write_db(file_name, db);


    // }
}

#[derive(Debug, Clone, PartialEq)]
struct Event {
    time: DateTime<Utc>,
    description: String,
    tag_ids: Vec<usize>,
}

#[derive(Debug)]
struct EventDB {
    events: Vec<Event>,
}

impl EventDB {
    fn new() -> EventDB {
        EventDB{
            events: vec!(),
        }
    }
}