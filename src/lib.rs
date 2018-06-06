#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;

extern crate chrono;

use chrono::prelude::*;
use std::{collections::{BTreeMap, HashMap},
          fs::File,
          io::{self, prelude::*, Error},
          path::Path};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub description: String,
    pub tag_ids: Vec<u16>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct EventDB {
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

    pub fn read(path: &Path) -> io::Result<EventDB> {
        // TODO: If the DB doesn't exist, create it.

        match File::open(path) {
            Ok(file) => {
                let event_db = serde_json::from_reader(file)?;
                return Ok(event_db);
            }
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    let event_db = EventDB::new();
                    event_db.write(path);
                    return Ok(event_db);
                } else {
                    return Err(e);
                }
            }
        }

        // let file = File::open(path)?;
        // let event_db = serde_json::from_reader(file)?;
    }

    pub fn write(&self, path: &Path) -> io::Result<()> {
        let file = File::create(path)?;
        serde_json::to_writer_pretty(&file, self)?;
        Ok(())
    }

    pub fn add_event(
        &mut self,
        time: i64,
        description: &str,
        short_names: &[&str],
    ) -> Result<(), &str> {
        let mut short_names = short_names.to_vec();
        short_names.sort();
        short_names.dedup();

        {
            let existing_short_names: Vec<_> = self.tags
                .iter()
                .map(|(_, v)| v.short_name.clone())
                .collect();

            let invalid_short_names: Vec<_> = short_names
                .iter()
                .filter(|sn| existing_short_names.contains(&sn.to_string()))
                .collect();
            if !invalid_short_names.is_empty() {
                return Err("Event contains at least one invalid short name (Tag)");
            }
        }

        let tag_ids: Vec<(_)> = short_names
            .iter()
            .flat_map(|sn| {
                self.tags
                    .iter()
                    .filter(|(_, v)| v.short_name == sn.to_string())
                    .map(|(k, _)| *k)
                    .collect::<Vec<u16>>()
            })
            .collect();

        let description = description.to_string();
        let event = Event {
            description,
            tag_ids,
        };
        self.events.insert(time, event);
        Ok(())
    }

    pub fn remove_event(&mut self, time: i64) -> Option<Event> {
        self.events.remove(&time)
    }

    pub fn add_tag(&mut self, mut tag: Tag) -> Result<(), &str> {
        if tag.short_name.is_empty() {
            return Err("You need to have a short name for the tag");
        }
        if tag.long_name.is_empty() {
            return Err("You need to have a long name for the tag");
        }
        for existing_tag in self.tags.values() {
            if existing_tag.short_name == tag.short_name {
                return Err("A tag with this short name already exists");
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

    pub fn remove_tag(&mut self, short_name: String) -> Result<(), &str> {
        // TODO: Remove the tag from all events it occurrs in before removing
        // it from the list.

        let to_remove: Vec<u16> = self.tags
            .iter()
            .filter(|&(_, ref val)| val.short_name == short_name)
            .map(|(key, _)| key.clone())
            .collect();

        if to_remove.is_empty() {
            return Err("That short name does not exist");
        }

        for key in to_remove {
            self.tags.remove(&key);
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct Tag {
    long_name: String,
    short_name: String,
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

        event_db
            .add_tag(Tag {
                long_name: "Zeroeth".to_string(),
                short_name: "zro".to_string(),
            })
            .unwrap();
        event_db
            .add_tag(Tag {
                long_name: "First".to_string(),
                short_name: "frs".to_string(),
            })
            .unwrap();
        event_db
            .add_tag(Tag {
                long_name: "Second".to_string(),
                short_name: "scn".to_string(),
            })
            .unwrap();

        // Adding a tag with a short name that already exists should not work.
        assert!(
            event_db
                .add_tag(Tag {
                    long_name: "Duplicate".to_string(),
                    short_name: "scn".to_string()
                })
                .is_err(),
            "Adding a duplicate tag didn't fail, but it should"
        );

        // Removing a tag should work.
        event_db.add_tag(Tag {
            long_name: "Remove this".to_string(),
            short_name: "rmv".to_string(),
        });
        assert!(
            event_db.remove_tag("rmv".to_string()).is_ok(),
            "Could not remove a tag"
        );

        event_db
            .add_event(
                time_now,
                "This event should be overwritten",
                &["zro", "frs", "scn"],
            )
            .unwrap();

        // Overwriting an existing event.
        event_db
            .add_event(time_now, "This event should exist", &["zro", "frs", "frs"])
            .unwrap();

        event_db
            .add_event(time_now + 1, "This is a description", &["scn"])
            .unwrap();

        // Adding and then removing an event.
        event_db
            .add_event(time_now + 2, "This event should be removed", &[])
            .unwrap();
        assert!(event_db.remove_event(time_now + 2).is_some());

        assert!(event_db.write(&file_name).is_ok());

        let event_db_read = EventDB::read(&file_name).unwrap();
        assert_eq!(event_db, event_db_read);
    }
}
