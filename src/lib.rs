#[macro_use]
extern crate serde_derive;
extern crate chrono;
extern crate serde;
extern crate serde_json;

#[cfg(test)]
extern crate quickcheck;

#[cfg(test)]
extern crate rand;

use chrono::prelude::*;
use std::{
    cmp::{max, min},
    collections::{BTreeMap, HashMap},
    error, fmt,
    fs::{self, File},
    io,
    path::Path,
};

// type Result<T> = std::result::Result<T, EventDbError>;

#[derive(Clone)]
pub struct EventDbError {
    error_kind: ErrorKind,
    message: String,
}

#[derive(Debug, Clone)]
pub enum ErrorKind {
    AlreadyExists,
    InvalidInput,
}

impl fmt::Display for EventDbError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}: {}", self.error_kind, self.message)
    }
}

impl fmt::Debug for EventDbError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}: {}", self.error_kind, self.message)
    }
}

impl std::error::Error for EventDbError {
    fn description(&self) -> &str {
        &self.message
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}

pub enum EventId {
    Timestamp(i64),
    Position(usize),
}

impl EventId {
    /// If there is a corresponding `Event` in the `EventDb` for this `EventId`, return the
    /// timestamp of the `EventId` as `Option<i64>`, otherwise return `None`.
    pub fn to_timestamp(&self, event_db: &EventDb) -> Option<i64> {
        match self {
            EventId::Timestamp(t) => match event_db.events.get(t) {
                Some(_) => Some(*t),
                None => None,
            },
            EventId::Position(pos) => event_db
                .events
                .iter()
                .rev()
                .nth(*pos)
                .map(|(time, _event)| *time),
        }
    }

    /// If there is a corresponding `Event` in the `EventDb` for this `EventId`, return the position
    /// of the `EventId` as `Option<usize>`. Otherwise return `None`.
    pub fn to_position(&self, event_db: &EventDb) -> Option<usize> {
        match self {
            EventId::Timestamp(t) => event_db
                .events
                .iter()
                .rev()
                .enumerate()
                .find(|(_, (time, _event))| t == *time)
                .map(|(i, (_, _))| i),
            EventId::Position(pos) => {
                let event = event_db
                    .events
                    .iter()
                    .rev()
                    .nth(*pos)
                    .map(|(_time, event)| event);

                match event {
                    Some(_) => Some(*pos),
                    None => None,
                }
            }
        }
    }

    pub fn exists(&self, event_db: &EventDb) -> bool {
        event_db.get_event(&self).is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub description: String,
    pub tag_ids: Vec<u16>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct Tag {
    pub long_name: String,
    pub short_name: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct EventDb {
    pub tags: HashMap<u16, Tag>,
    pub events: BTreeMap<i64, Event>,
}

#[derive(Debug)]
pub struct LogEvent {
    pub timestamp: i64,
    pub event: Event,
    pub duration: Option<i64>,
    pub position: usize,
}

impl EventDb {
    fn new() -> EventDb {
        EventDb {
            tags: HashMap::new(),
            events: BTreeMap::new(),
        }
    }

    pub fn read(path: &Path) -> io::Result<EventDb> {
        match File::open(path) {
            Ok(file) => {
                let event_db = serde_json::from_reader(file)?;
                Ok(event_db)
            }
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    let event_db = EventDb::new();
                    event_db.write(path)?;
                    Ok(event_db)
                } else {
                    Err(e)
                }
            }
        }
    }

    pub fn write(&self, path: &Path) -> io::Result<()> {
        let write_dir = path.parent().expect("Invalid database location");
        if !write_dir.exists() {
            fs::create_dir_all(write_dir)?;
        }

        let file = File::create(path)?;
        serde_json::to_writer_pretty(&file, self)?;
        Ok(())
    }

    pub fn add_event(
        &mut self,
        time: i64,
        description: &str,
        short_names: &[&str],
    ) -> Result<(), EventDbError> {
        let mut short_names = short_names.to_vec();
        short_names.sort();
        short_names.dedup();

        {
            let existing_short_names: Vec<_> = self
                .tags
                .iter()
                .map(|(_, v)| v.short_name.clone())
                .collect();

            let invalid_short_names: Vec<_> = short_names
                .iter()
                .filter(|sn| !existing_short_names.contains(&sn.to_string()))
                .collect();
            if !invalid_short_names.is_empty() {
                return Err(EventDbError {
                    error_kind: ErrorKind::InvalidInput,
                    message: format!(
                        "Event contains invalid short names: {:?}",
                        invalid_short_names
                    ),
                });
            }
        }

        let tag_ids: Vec<_> = short_names
            .iter()
            .flat_map(|sn| {
                self.tags
                    .iter()
                    .filter(|(_, v)| v.short_name == *sn)
                    .map(|(k, _)| *k)
                    .collect::<Vec<u16>>()
            }).collect();

        let description = description.to_string();
        let event = Event {
            description,
            tag_ids,
        };
        self.events.insert(time, event);
        Ok(())
    }

    /// Removes and returns the `Event` identified by the given `EventId`.
    pub fn remove_event(&mut self, event_id: &EventId) -> Option<Event> {
        let timestamp = event_id.to_timestamp(&self);

        if let Some(t) = timestamp {
            self.events.remove(&t)
        } else {
            None
        }
    }

    /// Returns an iterator over the `EventDb`'s `Tag`s.
    pub fn tags_iter(&self) -> std::collections::hash_map::Iter<u16, Tag> {
        self.tags.iter()
    }

    /// Takes a start `DateTime<Local>` and an end `DateTime<Local>` and returns a `Vec<LogEvent>`
    /// containing all `LogEvent`s between those two `DateTime<Local>`s.
    pub fn get_log_between_times(
        &self,
        time_start: &chrono::DateTime<Local>,
        time_end: &chrono::DateTime<Local>,
    ) -> Vec<LogEvent> {
        // let mut log_events = Vec<LogEvent>;

        let timestamp_early = min(time_start, time_end).timestamp();
        let timestamp_late = max(time_start, time_end).timestamp();

        self.events
            .iter()
            .rev()
            .filter(|&(time, _)| *time > timestamp_early && *time < timestamp_late)
            .map(|(time, event)| LogEvent {
                timestamp: *time,
                event: event.clone(),
                duration: self.get_event_duration(&EventId::Timestamp(*time)),
                position: self
                    .events
                    .iter()
                    .rev()
                    .position(|(t, _)| t == time)
                    .expect("Could not find an event at the given position"),
            }).collect()
    }

    /// Returns the `LogEvent` for the given `EventId`.
    pub fn get_log(&self, event_id: &EventId) -> Option<LogEvent> {
        let event = match self.get_event(event_id) {
            Some(x) => x,
            None => return None,
        };
        let duration = self.get_event_duration(&event_id);
        let timestamp = event_id.to_timestamp(&self).unwrap();
        let position = event_id.to_position(&self).unwrap();

        Some(LogEvent {
            timestamp,
            event: event.clone(),
            duration,
            position,
        })
    }

    /// Returns the event at the given `EventId`.
    pub fn get_event(&self, event_id: &EventId) -> Option<&Event> {
        match event_id.to_timestamp(self) {
            // Since `to_timestamp()` uses the `EventDb` to get `timestamp`, we can `unwrap()`
            // getting the `Event` at `timestamp` since we know it will exist.
            Some(timestamp) => Some(&self.events[&timestamp]),
            None => None,
        }
    }

    /// Gets the duration of the input `EventId`.
    pub fn get_event_duration(&self, event_id: &EventId) -> Option<i64> {
        if !event_id.exists(&self) {
            return None;
        }

        let current_event_timestamp = event_id.to_timestamp(&self).unwrap();
        let current_event_position = event_id.to_position(&self).unwrap();
        let preceeding_event_position = EventId::Position(current_event_position + 1);
        
        if let Some(preceeding_event_timestamp) = preceeding_event_position.to_timestamp(&self) {
            Some(current_event_timestamp - preceeding_event_timestamp)
        } else {
            Some(0)
        }
    }

    /// Returns a mutable reference to the `Event` identified by `EventId`.
    pub fn get_event_mut(&mut self, event_id: &EventId) -> Option<&mut Event> {
        match event_id.to_timestamp(self) {
            // Since `to_timestamp()` uses the `EventDb` to get `timestamp`, we can `unwrap()`
            // getting the `Event` at `timestamp` since we know it will exist.
            Some(timestamp) => Some(self.events.get_mut(&timestamp).unwrap()),
            None => None,
        }
    }

    pub fn add_tags_for_event(
        &mut self,
        event_id: &EventId,
        short_names: &[&str],
    ) -> Result<(), &str> {
        let mut tag_ids: Vec<u16> = vec![];

        for short_name in short_names {
            match self.tag_id_from_short_name(short_name) {
                Some(tag_id) => tag_ids.push(tag_id),
                None => return Err("Could not find a specified tag"),
            }
        }

        match self.get_event_mut(event_id) {
            Some(event) => {
                event.tag_ids.append(&mut tag_ids);
                event.tag_ids.sort();
                event.tag_ids.dedup();
                Ok(())
            }
            None => Err("Could not find an event at that `EventId`"),
        }
    }

    pub fn remove_tags_for_event(
        &mut self,
        event_id: &EventId,
        short_names: &[&str],
    ) -> Result<(), &str> {
        let mut tag_ids: Vec<u16> = vec![];

        for short_name in short_names {
            match self.tag_id_from_short_name(short_name) {
                Some(tag_id) => tag_ids.push(tag_id),
                None => return Err("Could not find a specified tag"),
            }
        }

        match self.get_event_mut(&event_id) {
            Some(event) => {
                for tag_id in &tag_ids {
                    let index = match event.tag_ids.iter().position(|i| *i == *tag_id) {
                        Some(i) => i,
                        None => continue,
                    };
                    event.tag_ids.remove(index);
                }

                Ok(())
            }
            None => Err("Could not find an event at that position"),
        }
    }

    pub fn add_tag(&mut self, long_name: &str, short_name: &str) -> Result<(), EventDbError> {
        let short_name = short_name.to_string();
        let long_name = long_name.to_string();

        if short_name.is_empty() {
            return Err(EventDbError {
                error_kind: ErrorKind::InvalidInput,
                message: "You need to have a short name for the tag".to_string(),
            });
        }
        if long_name.is_empty() {
            return Err(EventDbError {
                error_kind: ErrorKind::InvalidInput,
                message: "You need to have a long name for the tag".to_string(),
            });
        }
        for existing_tag in self.tags.values() {
            if existing_tag.short_name == short_name {
                return Err(EventDbError {
                    error_kind: ErrorKind::AlreadyExists,
                    message: "A tag with this short name already exists".to_string(),
                });
            }
        }

        // Ignoring lint because this needs to be a two-step process, and the lint doesn't
        // understand that.
        #[allow(unknown_lints)]
        #[allow(map_entry)]
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

    pub fn remove_tag(&mut self, short_name: &str) -> Result<(), EventDbError> {
        // Remove the tag from the database
        let key_to_remove: Vec<u16> = self
            .tags
            .iter()
            .filter(|&(_, ref val)| val.short_name == short_name)
            .map(|(key, _)| *key)
            .collect();

        if key_to_remove.is_empty() {
            return Err(EventDbError {
                error_kind: ErrorKind::InvalidInput,
                message: "That short name does not exist".to_string(),
            });
        }

        let key_to_remove = key_to_remove.first().unwrap();

        self.tags.remove(key_to_remove);

        // Remove the tag from all events where it's used.
        let affected_event_times: Vec<i64> = self
            .events
            .iter()
            .filter(|(_, e)| e.tag_ids.contains(key_to_remove))
            .map(|(t, _)| *t)
            .collect();

        for time in affected_event_times {
            let event = self.events.get_mut(&time).unwrap();
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
            .map(|(key, _)| *key)
            .next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::Arbitrary;
    use quickcheck::StdThreadGen;
    use rand::prelude::*;

    const LOW: usize = 1;
    const HIGH: usize = 20;

    #[test]
    fn quickcheck() {
        for _ in 0..100 {
            prop_event_db();
        }
    }

    fn prop_event_db() {
        let mut event_db = EventDb::new();
        let mut rng = thread_rng();
        let path = format!("test_files/generated/{}.json", rng.gen::<u16>());
        let event_db_path = Path::new(&path);

        for _ in 0..100 {
            match rng.gen_range(0, 5) {
                0 => qc_add_tag(&mut rng, &mut event_db),
                1 => qc_remove_tag(&mut rng, &mut event_db),
                2 => qc_write(&event_db_path, &event_db),
                3 => event_db = qc_read(&event_db_path),
                4 => qc_add_event(&mut rng, &mut event_db),
                _ => continue,
            };
        }
    }

    fn qc_add_event(rng: &mut rand::ThreadRng, event_db: &mut EventDb) {
        let time = rng.gen::<i64>();
        let description = &mut StdThreadGen::new(rng.gen_range(LOW, 100));
        let description = &String::arbitrary::<StdThreadGen>(description);

        let short_names_string: Vec<String> = (0..rng.gen_range(0, 10))
            .map(|_| get_random_short_name(&mut thread_rng(), &event_db))
            .filter(|i| i.is_some())
            .map(|i| i.unwrap())
            .collect();
        let short_names_str: Vec<&str> = short_names_string.iter().map(|i| i.as_str()).collect();

        event_db
            .add_event(time, description, short_names_str.as_slice())
            .unwrap();
    }

    fn qc_write(event_db_path: &Path, event_db: &EventDb) {
        event_db.write(event_db_path).unwrap();
    }

    fn qc_read(event_db_path: &Path) -> EventDb {
        EventDb::read(event_db_path).unwrap()
    }

    fn qc_add_tag(rng: &mut rand::ThreadRng, event_db: &mut EventDb) {
        let long_name = &mut StdThreadGen::new(rng.gen_range(LOW, HIGH));
        let long_name = &String::arbitrary::<StdThreadGen>(long_name);

        let short_name = &mut StdThreadGen::new(rng.gen_range(LOW, HIGH));
        let short_name = &String::arbitrary::<StdThreadGen>(short_name);

        if short_name.chars().count() == 0 || long_name.chars().count() == 0 {
            return;
        }

        if event_db.tag_id_from_short_name(short_name) != None {
            return;
        }

        event_db.add_tag(long_name, short_name).unwrap();
    }

    fn get_random_short_name(rng: &mut rand::ThreadRng, event_db: &EventDb) -> Option<String> {
        let tag_count = event_db.tags_iter().count();

        if tag_count == 0 {
            return None;
        }

        let short_name = event_db.tags_iter().nth(rng.gen_range(0, tag_count));

        if short_name.is_none() {
            None
        } else {
            Some(short_name.unwrap().1.short_name.to_string())
        }
    }

    fn qc_remove_tag(rng: &mut rand::ThreadRng, event_db: &mut EventDb) {
        let tag_count = event_db.tags_iter().count();

        if tag_count == 0 {
            return;
        }

        let short_name = event_db
            .tags_iter()
            .nth(rng.gen_range(0, tag_count))
            .expect("Could not find a tag at the given id")
            .1
            .short_name
            .to_owned();

        event_db.remove_tag(&short_name).unwrap();
    }

    #[test]
    /// Creates a simple database, writes it to a file, loads the written file
    /// and checks that the contents are the same as the original data.
    fn write_read_db() {
        let file_name = Path::new("test_files/read_write_test.json");
        let mut event_db = EventDb::new();

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
            event_db
                .add_tag("This tag should be removed", "rmv")
                .unwrap();
            event_db.add_event(time, description, &["rmv"]).unwrap();
            assert!(event_db.remove_tag("rmv").is_ok(), "Could not remove a tag");
            assert_eq!(
                *event_db.get_event(&EventId::Timestamp(time)).unwrap(),
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
            ).unwrap();

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
        assert!(
            event_db
                .remove_event(&EventId::Timestamp(time_now + 2))
                .is_some()
        );

        assert!(event_db.write(&file_name).is_ok());

        let event_db_read = EventDb::read(&file_name).unwrap();
        assert_eq!(event_db, event_db_read);
    }
}
