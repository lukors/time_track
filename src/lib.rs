#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;

extern crate chrono;

use chrono::prelude::*;
use std::{fs::File,
          io::{self, prelude::*},
          path::Path};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_event() {
        let event = Event {
            time: UnixTime::from_timestamp(Utc::now().timestamp()),
            description: "This is a description".to_string(),
            tag_ids: vec![],
        };

        let mut event_db = EventDB::new();
        event_db.events.push(event.clone());

        assert_eq!(event, event_db.events[0]);
    }

    #[test]
    fn write_db() {
        let file_name = "target/test_files/write_test.txt";
        let mut event_db = EventDB::new();
        // TODO: Add some things to the DB that can be written.
        let time_now = Utc::now().timestamp();
        event_db.events.push(Event {
            time: UnixTime { time: time_now },
            description: "write one, should be over-written due to exact same time".to_string(),
            tag_ids: vec![2, 1, 4],
        });
        event_db.events.push(Event {
            time: UnixTime { time: time_now },
            description:
                "write two, should be visible since it has the exact same time as previous write"
                    .to_string(),
            tag_ids: vec![2, 1, 4],
        });
        event_db.events.push(Event {
            time: UnixTime { time: time_now + 1 },
            description: "one second later".to_string(),
            tag_ids: vec![2, 1, 4],
        });

        assert!(super::write_db(&event_db, Path::new(file_name)).is_ok());
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

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
struct UnixTime {
    time: i64,
}

impl UnixTime {
    fn from_timestamp(secs: i64) -> UnixTime {
        UnixTime { time: secs }
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
        EventDB { events: vec![] }
    }
}

fn write_db(event_db: &EventDB, path: &Path) -> io::Result<()> {
    let file = File::create(path)?;
    for event in &event_db.events {
        serde_json::to_writer_pretty(&file, &event)?;
    }
    // file.write_all(serialized)?;
    Ok(())
}
