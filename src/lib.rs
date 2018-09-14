#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;
extern crate chrono;

#[cfg(test)]
#[macro_use]
extern crate quickcheck;

#[cfg(test)]
extern crate rand;

use chrono::{prelude::*,
             Duration};
use std::{cmp::{min, max},
          collections::{BTreeMap, HashMap},
          error,
          fmt,
          fs::{self,
               File},
          io,
          path::Path};

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

    fn cause(&self) -> Option<&error::Error> {
        // Generic error, underlying cause isn't tracked.
        None
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
                return Ok(event_db);
            }
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    let event_db = EventDb::new();
                    event_db.write(path)?;
                    return Ok(event_db);
                } else {
                    return Err(e);
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
            let existing_short_names: Vec<_> = self.tags
                .iter()
                .map(|(_, v)| v.short_name.clone())
                .collect();

            let invalid_short_names: Vec<_> = short_names
                .iter()
                .filter(|sn| !existing_short_names.contains(&sn.to_string()))
                .collect();
            if !invalid_short_names.is_empty() {
                return Err(EventDbError{
                    error_kind: ErrorKind::InvalidInput,
                    message: format!("Event contains invalid short names: {:?}"
                        , invalid_short_names)
                })
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

    pub fn get_event_from_pos(&self, position: usize) -> Option<(i64, &Event)> {
        self.events.iter().rev().nth(position).map(|(time, event)| (*time, event))
    }

    fn get_event_from_pos_mut(&mut self, position: usize) -> Option<(i64, &mut Event)> {
        self.events.iter_mut().rev().nth(position).map(|(time, event)| (*time, event))
    }

    pub fn tags_iter(&self) -> std::collections::hash_map::Iter<u16, Tag> {
        self.tags.iter()
    }

    /// Takes a start and end date and returns a vector of information about
    /// the events on and between those dates.
    pub fn get_log_between_times(&self, time_start: &chrono::DateTime<Local>, time_end: &chrono::DateTime<Local>) -> Vec<LogEvent> {
        // let mut log_events = Vec<LogEvent>;
        
        let timestamp_early = min(time_start, time_end).timestamp();
        let timestamp_late = max(time_start, time_end).timestamp();

        self.events
            .iter()
            .rev()
            .filter(|&(time, _)| {time > &timestamp_early && time < &timestamp_late})
            .map(|(time, event)| {
                LogEvent{
                    timestamp: time.clone(),
                    event: event.clone(),
                    duration: self.get_event_duration(*time),
                    position: self.events
                        .iter()
                        .rev()
                        .position(|(t, _)| t == time)
                        .expect("Could not find an event at the given position"),
                }
            })
            .collect()
    }

    pub fn get_log_from_pos(&self, position: usize) -> Option<LogEvent> {
        let (timestamp, event) = match self.get_event_from_pos(position) {
            Some(x) => x,
            None => return None,
        };
        let duration = self.get_event_duration(timestamp);

        Some(LogEvent{
            timestamp,
            event: event.clone(),
            duration,
            position,
        })
    }

    /// Returns the duration of the given event, given in seconds.
    pub fn get_event_duration_from_pos(&self, position: usize) -> Option<i64> {
        let time = match self.get_event_from_pos(position) {
            Some(event) => event.0,
            None => return None,
        };

        self.get_event_duration(time)
    }

    pub fn get_event_duration(&self, time: i64) -> Option<i64> {
        let preceding_event_position =
            match self.events.iter().position(|(t, _)| t == &time) {
                Some(t) => t,
                None => return None,
        };

        if preceding_event_position == 0 {
            return None
        }

        let preceeding_event_time = match self.events
            .iter()
            .nth(preceding_event_position - 1)
            .map(|(time, _)| *time) {
                Some(t) => t,
                None => return None,
        };

        Some(time - preceeding_event_time)
    }

    pub fn get_event_mut(&mut self, position: usize) -> Option<&mut Event> {
        match self.get_event_from_pos_mut(position) {
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

        match self.get_event_from_pos_mut(position) {
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

        match self.get_event_from_pos_mut(position) {
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

    pub fn add_tag(&mut self, long_name: &str, short_name: &str) -> Result<(), EventDbError> {
        let short_name = short_name.to_string();
        let long_name = long_name.to_string();

        if short_name.is_empty() {
            return Err(
                EventDbError {
                    error_kind: ErrorKind::InvalidInput,
                    message: "You need to have a short name for the tag".to_string(),
                }
            )
        }
        if long_name.is_empty() {
            return Err(
                EventDbError {
                    error_kind: ErrorKind::InvalidInput,
                    message: "You need to have a long name for the tag".to_string(),
                }
            )
        }
        for existing_tag in self.tags.values() {
            if existing_tag.short_name == short_name {
                return Err(
                    EventDbError {
                        error_kind: ErrorKind::AlreadyExists,
                        message: "A tag with this short name already exists".to_string(),
                    }
                )
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

    pub fn remove_tag(&mut self, short_name: String) -> Result<(), EventDbError> {
        // Remove the tag from the database
        let key_to_remove: Vec<u16> = self.tags
            .iter()
            .filter(|&(_, ref val)| val.short_name == short_name)
            .map(|(key, _)| key.clone())
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::prelude::*;
    use quickcheck::TestResult;
    use quickcheck::Arbitrary;
    use quickcheck::StdThreadGen;
    use rand::prelude::*;

    const LOW: usize = 1;
    const HIGH: usize = 20;

    #[test]
    fn quickcheck() {
        for i in 0..100 {
            prop_event_db();
        }
    }

    enum TtTestResult {
        Pass,
        Fail,
        Discard,
        Error(String),
    }

    struct TtTestCount {
        pass: i32,
        discard: i32,
    }

    fn prop_event_db() {
        let mut event_db = EventDb::new();
        let mut rng = thread_rng();
        let path = format!("test_files/generated/{}.json", rng.gen::<u16>());
        let event_db_path = Path::new(&path);

        for i in 0..100 {
            match rng.gen_range(0, 5) {
                0 => qc_add_tag(&mut rng, &mut event_db),
                1 => qc_remove_tag(&mut rng, &mut event_db),
                2 => qc_write(&event_db_path, &event_db),
                3 => event_db = qc_read(&event_db_path, &event_db),
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
            .map(|_| get_random_short_name(&mut thread_rng(), &event_db) )
            .filter(|i| i.is_some())
            .map(|i| i.unwrap())
            .collect();
        let short_names_str: Vec<&str> = short_names_string
            .iter()
            .map(|i| i.as_str())
            .collect();

        event_db.add_event(
            time,
            description,
            short_names_str.as_slice(),
        ).unwrap();
    }

    fn qc_write(event_db_path: &Path, event_db: &EventDb) {
        event_db.write(event_db_path).unwrap();
    }

    fn qc_read(event_db_path: &Path, event_db: &EventDb) -> EventDb {
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
            return None
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
            return
        }

        let short_name = (event_db.tags_iter()
            .nth(rng.gen_range(0, tag_count))
            .expect("Could not find a tag at the given id")
            .1.short_name.to_string());

        event_db.remove_tag(short_name.to_string()).unwrap();
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

        let event_db_read = EventDb::read(&file_name).unwrap();
        assert_eq!(event_db, event_db_read);
    }
}
