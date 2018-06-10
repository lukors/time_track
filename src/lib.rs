#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;

extern crate chrono;

use std::{collections::{BTreeMap, HashMap},
          fs::File,
          io,
          path::Path};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub description: String,
    pub tag_ids: Vec<u16>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct EventDB {
    pub tags: HashMap<u16, Tag>,
    pub events: BTreeMap<i64, Event>,
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
                    event_db.write(path)?;
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
    ) -> Result<(), String> {
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
                .filter(|sn| !existing_short_names.contains(&sn.to_string()))
                .collect();
            if !invalid_short_names.is_empty() {
                return Err(format!(
                    "Event contains invalid short names: {:?}",
                    invalid_short_names
                ));
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

    pub fn remove_event(&mut self, position: usize) -> Option<Event> {
        let time_to_remove = self.events
            .iter()
            .rev()
            .nth(position)
            .map(|(time, _)| *time);

        if let Some(time) = time_to_remove {
            return self.remove_event_time(time)
        }
        None
    }

    pub fn remove_event_time(&mut self, time: i64) -> Option<Event> {
        self.events.remove(&time)
    }

    fn event_from_pos(&self, position: usize) -> Option<(i64, &Event)> {
        self.events.iter().rev().nth(position).map(|(time, event)| (*time, event))
    }

    fn event_from_pos_mut(&mut self, position: usize) -> Option<(i64, &mut Event)> {
        self.events.iter_mut().rev().nth(position).map(|(time, event)| (*time, event))
    }

    pub fn get_event(&self, position: usize) -> Option<&Event> {
        match self.event_from_pos(position) {
            Some((_, event)) => Some(event),
            None => None,
        }
    }

    pub fn get_event_mut(&mut self, position: usize) -> Option<&mut Event> {
        match self.event_from_pos_mut(position) {
            Some((_, event)) => Some(event),
            None => None,
        }
    }

    pub fn get_event_time(&self, time: i64) -> Option<&Event> {
        self.events.get(&time)
    }

    pub fn add_tags_for_event(&mut self, position: usize, short_names: &[&str]) -> Result<(), &str> {
        let mut tag_ids: Vec<u16> = vec![];

        for short_name in short_names {
            match self.tag_id_from_short_name(short_name) {
                Some(tag_id) => tag_ids.push(tag_id),
                None => return Err("Could not find a specified tag"),
            }
        }

        match self.event_from_pos_mut(position) {
            Some((_, event)) => {
                event.tag_ids.append(&mut tag_ids);
                event.tag_ids.sort();
                event.tag_ids.dedup();
                Ok(())
            },
            None => Err("Could not find an event at that position"),
        }
    }

    pub fn remove_tags_for_event(&mut self, position: usize, short_names: &[&str]) -> Result<(), &str> {
        let mut tag_ids: Vec<u16> = vec![];

        for short_name in short_names {
            match self.tag_id_from_short_name(short_name) {
                Some(tag_id) => tag_ids.push(tag_id),
                None => return Err("Could not find a specified tag"),
            }
        }

        match self.event_from_pos_mut(position) {
            Some((_, event)) => {
                for tag_id in &tag_ids {
                    let index = match event.tag_ids.iter().position(|i| *i == *tag_id) {
                        Some(i) => i,
                        None => continue,
                    };
                    event.tag_ids.remove(index);
                }

                Ok(())
            },
            None => Err("Could not find an event at that position"),
        }
    }

    pub fn add_tag(&mut self, long_name: &str, short_name: &str) -> Result<(), &str> {
        let short_name = short_name.to_string();
        let long_name = long_name.to_string();

        if short_name.is_empty() {
            return Err("You need to have a short name for the tag");
        }
        if long_name.is_empty() {
            return Err("You need to have a long name for the tag");
        }
        for existing_tag in self.tags.values() {
            if existing_tag.short_name == short_name {
                return Err("A tag with this short name already exists");
            }
        }

        for number in 0.. {
            if !self.tags.contains_key(&number) {
                self.tags.insert(
                    number,
                    Tag {
                        short_name,
                        long_name,
                    },
                );
                break;
            }
        }

        Ok(())
    }

    pub fn remove_tag(&mut self, short_name: String) -> Result<(), &str> {
        // TODO: Remove the tag from all events it occurrs in before removing
        // it from the list.

        // Remove the tag from the database
        let key_to_remove: Vec<u16> = self.tags
            .iter()
            .filter(|&(_, ref val)| val.short_name == short_name)
            .map(|(key, _)| key.clone())
            .collect();

        if key_to_remove.is_empty() {
            return Err("That short name does not exist");
        }

        let key_to_remove = key_to_remove.first().unwrap();

        self.tags.remove(key_to_remove);

        // Remove the tag from all events where it's used.
        let affected_event_times: Vec<i64> = self.events
            .iter()
            .filter(|(_, e)| e.tag_ids.contains(key_to_remove))
            .map(|(t, _)| *t)
            .collect();

        for time in affected_event_times {
            let mut event = self.events.get_mut(&time).unwrap();
            let mut index_to_remove: Option<u16> = None;

            for (i, tag_id) in event.tag_ids.iter().enumerate() {
                if tag_id == key_to_remove {
                    index_to_remove = Some(i as u16);
                    break;
                }
            }

            if let Some(i) = index_to_remove {
                event.tag_ids.remove(i as usize);
            }
        }

        Ok(())
    }

    pub fn tag_id_from_short_name(&self, short_name: &str) -> Option<u16> {
        self.tags 
            .iter()
            .filter(|&(_, ref val)| val.short_name == short_name)
            .map(|(key, _)| key.clone())
            .next()
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct Tag {
    pub long_name: String,
    pub short_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::prelude::*;

    #[test]
    /// Creates a simple database, writes it to a file, loads the written file
    /// and checks that the contents are the same as the original data.
    fn write_read_db() {
        let file_name = Path::new("test_files/read_write_test.json");
        let mut event_db = EventDB::new();

        let time_now = Utc::now().timestamp();

        event_db.add_tag("Zeroeth", "zro").unwrap();
        event_db.add_tag("First", "frs").unwrap();
        event_db.add_tag("Second", "scn").unwrap();

        // Adding a tag with a short name that already exists should not work.
        assert!(
            event_db.add_tag("Duplicate", "scn").is_err(),
            "Adding a duplicate tag didn't fail, but it should"
        );

        // Removing a tag should work.
        {
            let time = time_now + 10;
            let description = "This event should have no tags";
            event_db.add_tag("This tag should be removed", "rmv");
            event_db.add_event(time, description, &["rmv"]);
            assert!(
                event_db.remove_tag("rmv".to_string()).is_ok(),
                "Could not remove a tag"
            );
            assert_eq!(
                *event_db.get_event_time(time).unwrap(),
                Event {
                    description: description.to_string(),
                    tag_ids: vec![],
                }
            );
        }

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
        assert!(event_db.remove_event_time(time_now + 2).is_some());

        assert!(event_db.write(&file_name).is_ok());

        let event_db_read = EventDB::read(&file_name).unwrap();
        assert_eq!(event_db, event_db_read);
    }
}
