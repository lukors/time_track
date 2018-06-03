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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Event {
    description: String,
    tag_ids: Vec<usize>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
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
    // for (time, event) in &event_db.events {
    //     let to_write = (time, &event.description, &event.tag_ids);
    // }
    serde_json::to_writer_pretty(&file, &event_db)?;
    Ok(())
}

fn read_db(path: &Path) -> io::Result<EventDB> {
    let file = File::open(path)?;
    let event_db = serde_json::from_reader(file)?;
    Ok(event_db)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_read_db() {
        let file_name = Path::new("test_files/read_write_test.json");
        let mut event_db = EventDB::new();
        
        let time_now = Utc::now().timestamp();
        let description = "This event should not exist".to_string();
        let tag_ids = vec![2, 1, 4];
        
        event_db.events.insert(
            time_now,
            Event {
            description,
            tag_ids,
        });

        let description = "This is a description".to_string();
        let tag_ids = vec![3, 1, 1];
        
        event_db.events.insert(
            time_now + 1,
            Event {
            description,
            tag_ids,
        });

        let description = "This event should exist".to_string();
        let tag_ids = vec![];
        
        event_db.events.insert(
            time_now,
            Event {
            description,
            tag_ids,
        });

        assert!(super::write_db(&event_db, &file_name).is_ok());

        
        let event_db_read = super::read_db(&file_name).unwrap();
        assert_eq!(event_db, event_db_read);
    }
}
