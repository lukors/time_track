#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;

extern crate chrono;

use chrono::prelude::*;
use std::{
    path::Path,
    io,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_event() {
        let event = Event{
            time: UnixTime::from_timestamp(Utc::now().timestamp()),
            description: "This is a description".to_string(),
            tag_ids: vec!(),
        };

        let mut event_db = EventDB::new();
        event_db.events.push(event.clone());

        assert_eq!(event, event_db.events[0]);
    }

    #[test]
    fn write_db() {
        let file_name = "write_test.txt";
        let event_db = EventDB::new();
        // TODO: Add some things to the DB that can be written.
        
        //assert!(super::write_db(event_db, Path::new(file_name)).is_ok());
    }

    // TODO: Test for reading DB
    // #[test]
    // fn read_db() {
    //     let file_name = "write_test.txt";
    //     let event_db = EventDB::new();
    //     let target_contents = "whatever should be in the file";
        
    //     super::write_db(event_db, file_name);
        
    //     // TODO: Read the file into a variable to be compared in the assert_eq.
    //     // let written_contents = file.read(file_name);

    //     assert_eq!(target_contents, written_contents);
    // }

    // TODO: Test for reading and writing DB:
    // #[test]
    // fn write_read_db() {
    //     let file_name = "write_test.txt";
    //     let event_db = EventDB::new();
    //     let target_contents = "whatever should be in the file";
        
    //     super::write_db(event_db, file_name);
        
    //     // TODO: Read the file into a variable to be compared in the assert_eq.
    //     // let written_contents = file.read(file_name);

    //     assert_eq!(target_contents, written_contents);
    // }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct UnixTime {
    time: i64,
}

impl UnixTime {
    fn from_timestamp(secs: i64) -> UnixTime {
        UnixTime{
            time: secs,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Event {
    time: UnixTime,
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

// fn write_db(event_db: EventDB, path: &Path) -> io::Result<()> {

// }