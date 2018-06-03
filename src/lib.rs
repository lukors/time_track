#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;

extern crate chrono;

use chrono::prelude::*;
use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, prelude::*},
    path::Path};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_db() {
        let file_name = "test_files/write_test.txt";
        let mut event_db = EventDB::new();
        let time_now = Utc::now().timestamp();
        event_db.events.insert(
            time_now,
            Event {
            description: "write one, should be over-written due to exact same time".to_string(),
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Event {
    description: String,
    tag_ids: Vec<usize>,
}

#[derive(Debug)]
struct EventDB {
    events: BTreeMap<i64, Event>,
}

impl EventDB {
    fn new() -> EventDB {
        EventDB { events: BTreeMap::new() }
    }
}

fn write_db(event_db: &EventDB, path: &Path) -> io::Result<()> {
    let file = File::create(path)?;
    for (time, event) in &event_db.events {
        let to_write = (time, &event.description, &event.tag_ids);
        serde_json::to_writer_pretty(&file, &to_write)?;
    }
    Ok(())
}
