#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;

extern crate chrono;

use chrono::prelude::*;
use std::{collections::{BTreeMap, HashMap},
          fs::File,
          io::{self, prelude::*},
          path::Path};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Event {
    description: String,
    tag_ids: Vec<u16>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct EventDB {
    tags: HashMap<u16, Tag>,
    events: BTreeMap<i64, Event>,
}

impl EventDB {
    fn new() -> EventDB {
        EventDB {
            tags: HashMap::new(),
            events: BTreeMap::new(),
        }
    }

    fn add_event(&mut self, time: i64, mut event: Event) -> Result<(), &str> {
        for tag in &event.tag_ids {
            if !self.tags.contains_key(tag) {
                return Err("The event contains a tag that does not exist")
            }
        }

        event.tag_ids.sort();
        event.tag_ids.dedup();

        self.events.insert(time, event);
        Ok(())
    }

    fn remove_event(&mut self, time: i64) -> Option<Event> {
        self.events.remove(&time)
    }

    fn add_tag(&mut self, mut tag: Tag) -> Result<(), &str> {
        if tag.short_name.is_empty() {
            return Err("You need to have a short name for the tag")
        }
        if tag.long_name.is_empty() {
            return Err("You need to have a long name for the tag")
        }
        for existing_tag in self.tags.values() {
            if existing_tag.short_name == tag.short_name{
                return Err("A tag with this short name already exists")
            }
        }

        for number in 0.. {
            if !self.tags.contains_key(&number) {
                self.tags.insert(number, tag);
                break;
            }
        }

        Ok(())
    }

    // fn remove_tag(&mut self, )
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct Tag {
    long_name: String,
    short_name: String,
}

fn write_db(event_db: &EventDB, path: &Path) -> io::Result<()> {
    let file = File::create(path)?;
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
    /// Creates a simple database, writes it to a file, loads the written file
    /// and checks that the contents are the same as the original data.
    fn write_read_db() {
        let file_name = Path::new("test_files/read_write_test.json");
        let mut event_db = EventDB::new();

        let time_now = Utc::now().timestamp();

        event_db.add_tag(Tag{long_name: "Zeroeth".to_string(), short_name: "zro".to_string()}).unwrap();
        event_db.add_tag(Tag{long_name: "First".to_string(), short_name: "frs".to_string()}).unwrap();
        event_db.add_tag(Tag{long_name: "Second".to_string(), short_name: "scn".to_string()}).unwrap();
        
        // Adding a tag with a short name that already exists should not work.
        assert!(event_db.add_tag(Tag{long_name: "Duplicate".to_string(), short_name: "scn".to_string()}).is_err());

        event_db.add_event(time_now, Event {
                description: "This event should be overwritten".to_string(),
                tag_ids: vec![0, 1, 2],
            },
        ).unwrap();

        // Overwriting an existing event.
        event_db.add_event(time_now, Event {
                description: "This event should exist".to_string(),
                tag_ids: vec![0, 1, 1],
            },
        ).unwrap();

        event_db.add_event(time_now + 1, Event {
                description: "This is a description".to_string(),
                tag_ids: vec![2],
            },
        ).unwrap();

        // Adding and then removing an event.
        event_db.add_event(time_now + 2, Event {
                description: "This event should be removed".to_string(),
                tag_ids: vec![],
            },
        ).unwrap();
        event_db.remove_event(time_now + 2);

        assert!(super::write_db(&event_db, &file_name).is_ok());

        let event_db_read = super::read_db(&file_name).unwrap();
        assert_eq!(event_db, event_db_read);
    }
}
