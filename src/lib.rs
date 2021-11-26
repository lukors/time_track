#[macro_use]
extern crate serde_derive;
extern crate chrono;
extern crate serde;
extern crate serde_json;

use chrono::prelude::*;
use std::{
    cmp::{max, min},
    collections::BTreeMap,
    error,
    fmt::{self, Display},
    fs::{self, File},
    io,
    path::Path,
};

#[derive(Clone)]
pub struct CheckpointDbError {
    error_kind: ErrorKind,
    message: String,
}

#[derive(Debug, Clone)]
pub enum ErrorKind {
    AlreadyExists,
    InvalidInput,
}

impl fmt::Display for CheckpointDbError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}: {}", self.error_kind, self.message)
    }
}

impl fmt::Debug for CheckpointDbError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}: {}", self.error_kind, self.message)
    }
}

impl std::error::Error for CheckpointDbError {
    fn description(&self) -> &str {
        &self.message
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}

#[derive(Clone, Copy, Debug)]
pub enum CheckpointId {
    Timestamp(i64),
    Position(usize),
}

impl CheckpointId {
    /// If there is a corresponding `Checkpoint` in the `CheckpointDb` for this `CheckpointId`, return the
    /// timestamp of the `CheckpointId` as `Option<i64>`, otherwise return `None`.
    pub fn to_timestamp(&self, checkpoint_db: &CheckpointDb) -> Option<i64> {
        match self {
            CheckpointId::Timestamp(t) => checkpoint_db.checkpoints.get(t).map(|_| *t),
            CheckpointId::Position(pos) => checkpoint_db
                .checkpoints
                .iter()
                .rev()
                .nth(*pos)
                .map(|(time, _checkpoint)| *time),
        }
    }

    /// If there is a corresponding `Checkpoint` in the `CheckpointDb` for this `CheckpointId`, return the position
    /// of the `CheckpointId` as `Option<usize>`. Otherwise return `None`.
    pub fn to_position(&self, checkpoint_db: &CheckpointDb) -> Option<usize> {
        match self {
            CheckpointId::Timestamp(t) => checkpoint_db
                .checkpoints
                .iter()
                .rev()
                .enumerate()
                .find(|(_, (time, _checkpoint))| t == *time)
                .map(|(i, (_, _))| i),
            CheckpointId::Position(pos) => {
                let checkpoint = checkpoint_db
                    .checkpoints
                    .iter()
                    .rev()
                    .nth(*pos)
                    .map(|(_time, checkpoint)| checkpoint);

                checkpoint.map(|_| *pos)
            }
        }
    }

    pub fn exists(&self, checkpoint_db: &CheckpointDb) -> bool {
        checkpoint_db.get_checkpoint(self).is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ProjectId {
    NoId,
    Id(u16),
}

impl Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoId => write!(f, "No ID"),
            Self::Id(id) => write!(f, "{}", id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub message: String,
    pub project_id: ProjectId,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct Project {
    pub long_name: String,
    pub short_name: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct CheckpointDb {
    pub projects: BTreeMap<u16, Project>,
    pub checkpoints: BTreeMap<i64, Checkpoint>,
}

#[derive(Debug)]
pub struct LogCheckpoint {
    pub timestamp: i64,
    pub checkpoint: Checkpoint,
    pub duration: Option<i64>,
    pub position: usize,
}

impl CheckpointDb {
    fn new() -> CheckpointDb {
        CheckpointDb {
            projects: BTreeMap::new(),
            checkpoints: BTreeMap::new(),
        }
    }

    pub fn read(path: &Path) -> io::Result<CheckpointDb> {
        match File::open(path) {
            Ok(file) => {
                let checkpoint_db = serde_json::from_reader(file)?;
                Ok(checkpoint_db)
            }
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    let checkpoint_db = CheckpointDb::new();
                    checkpoint_db.write(path)?;
                    Ok(checkpoint_db)
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

    pub fn add_checkpoint(
        &mut self,
        time: i64,
        message: &str,
        project_id: ProjectId,
    ) -> Result<(), CheckpointDbError> {
        if let ProjectId::Id(project_id) = project_id {
            if !self.projects.contains_key(&project_id) {
                return Err(CheckpointDbError {
                    error_kind: ErrorKind::InvalidInput,
                    message: "the given project id does not exist".to_string(),
                });
            }
        }

        let message = message.to_string();
        let checkpoint = Checkpoint {
            message,
            project_id,
        };
        self.checkpoints.insert(time, checkpoint);
        Ok(())
    }

    /// Removes and returns the `Checkpoint` identified by the given `CheckpointId`.
    pub fn remove_checkpoint(&mut self, checkpoint_id: &CheckpointId) -> Option<Checkpoint> {
        let timestamp = checkpoint_id.to_timestamp(self);

        if let Some(t) = timestamp {
            self.checkpoints.remove(&t)
        } else {
            None
        }
    }

    /// Takes a start `DateTime<Local>` and an end `DateTime<Local>` and returns a `Vec<LogCheckpoint>`
    /// containing all `LogCheckpoint`s between those two `DateTime<Local>`s.
    pub fn get_log_between_times(
        &self,
        time_start: &chrono::DateTime<Local>,
        time_end: &chrono::DateTime<Local>,
    ) -> Vec<LogCheckpoint> {
        let timestamp_early = min(time_start, time_end).timestamp();
        let timestamp_late = max(time_start, time_end).timestamp();

        self.checkpoints
            .iter()
            .rev()
            .filter(|&(time, _)| *time > timestamp_early && *time < timestamp_late)
            .map(|(time, checkpoint)| LogCheckpoint {
                timestamp: *time,
                checkpoint: checkpoint.clone(),
                duration: self.get_checkpoint_duration(&CheckpointId::Timestamp(*time)),
                position: self
                    .checkpoints
                    .iter()
                    .rev()
                    .position(|(t, _)| t == time)
                    .expect("Could not find an checkpoint at the given position"),
            })
            .collect()
    }

    /// Returns the `LogCheckpoint` for the given `CheckpointId`.
    pub fn get_log(&self, checkpoint_id: &CheckpointId) -> Option<LogCheckpoint> {
        let checkpoint = match self.get_checkpoint(checkpoint_id) {
            Some(x) => x,
            None => return None,
        };
        let duration = self.get_checkpoint_duration(checkpoint_id);
        let timestamp = checkpoint_id.to_timestamp(self).unwrap();
        let position = checkpoint_id.to_position(self).unwrap();

        Some(LogCheckpoint {
            timestamp,
            checkpoint: checkpoint.clone(),
            duration,
            position,
        })
    }

    /// Returns the checkpoint at the given `CheckpointId`.
    pub fn get_checkpoint(&self, checkpoint_id: &CheckpointId) -> Option<&Checkpoint> {
        match checkpoint_id.to_timestamp(self) {
            Some(timestamp) => Some(&self.checkpoints[&timestamp]),
            None => None,
        }
    }

    /// Gets the duration of the input `CheckpointId`.
    pub fn get_checkpoint_duration(&self, checkpoint_id: &CheckpointId) -> Option<i64> {
        if !checkpoint_id.exists(self) {
            return None;
        }

        let current_checkpoint_timestamp = checkpoint_id.to_timestamp(self).unwrap();
        let current_checkpoint_position = checkpoint_id.to_position(self).unwrap();
        let preceeding_checkpoint_position =
            CheckpointId::Position(current_checkpoint_position + 1);

        if let Some(preceeding_checkpoint_timestamp) =
            preceeding_checkpoint_position.to_timestamp(self)
        {
            Some(current_checkpoint_timestamp - preceeding_checkpoint_timestamp)
        } else {
            Some(0)
        }
    }

    /// Returns a mutable reference to the `Checkpoint` identified by `CheckpointId`.
    pub fn get_checkpoint_mut(&mut self, checkpoint_id: &CheckpointId) -> Option<&mut Checkpoint> {
        match checkpoint_id.to_timestamp(self) {
            Some(timestamp) => self.checkpoints.get_mut(&timestamp),
            None => None,
        }
    }

    pub fn set_checkpoint_project(
        &mut self,
        checkpoint_id: CheckpointId,
        project_id: ProjectId,
    ) -> Result<(), CheckpointDbError> {
        if let ProjectId::Id(project_id) = project_id {
            if !self.projects.contains_key(&project_id) {
                return Err(CheckpointDbError {
                    error_kind: ErrorKind::InvalidInput,
                    message: "could not find the given project_id".to_string(),
                });
            }
        }

        if let Some(checkpoint) = self.get_checkpoint_mut(&checkpoint_id) {
            checkpoint.project_id = project_id;
            Ok(())
        } else {
            Err(CheckpointDbError {
                error_kind: ErrorKind::InvalidInput,
                message: "could not find the given checkpoint_id".to_string(),
            })
        }
    }

    pub fn add_project(
        &mut self,
        long_name: &str,
        short_name: &str,
    ) -> Result<ProjectId, CheckpointDbError> {
        let short_name = short_name.to_string();
        let long_name = long_name.to_string();

        if short_name.is_empty() {
            return Err(CheckpointDbError {
                error_kind: ErrorKind::InvalidInput,
                message: "You need to have a short name for the project".to_string(),
            });
        }
        if long_name.is_empty() {
            return Err(CheckpointDbError {
                error_kind: ErrorKind::InvalidInput,
                message: "You need to have a long name for the project".to_string(),
            });
        }
        for existing_project in self.projects.values() {
            if existing_project.short_name == short_name {
                return Err(CheckpointDbError {
                    error_kind: ErrorKind::AlreadyExists,
                    message: "A project with this short name already exists".to_string(),
                });
            }
        }

        let mut project_id = ProjectId::NoId;

        // Ignoring lint because this needs to be a two-step process, and the lint doesn't
        // understand that.
        #[allow(unknown_lints)]
        #[allow(clippy::map_entry)]
        for number in 0.. {
            if !self.projects.contains_key(&number) {
                self.projects.insert(
                    number,
                    Project {
                        short_name,
                        long_name,
                    },
                );
                project_id = ProjectId::Id(number);
                break;
            }
        }

        Ok(project_id)
    }

    pub fn remove_project(&mut self, project_id: ProjectId) -> Result<(), CheckpointDbError> {
        if let ProjectId::Id(project_id) = project_id {
            self.projects.remove(&project_id);
        } else {
            return Err(CheckpointDbError {
                error_kind: ErrorKind::InvalidInput,
                message: "That ProjectId does not exist".to_string(),
            });
        }

        // Remove the project from all checkpoints where it's used.
        let affected_checkpoint_times: Vec<i64> = self
            .checkpoints
            .iter()
            .filter(|(_, e)| project_id == e.project_id)
            .map(|(t, _)| *t)
            .collect();

        for time in affected_checkpoint_times {
            let checkpoint = self.checkpoints.get_mut(&time).unwrap();
            checkpoint.project_id = ProjectId::NoId;
        }

        Ok(())
    }

    pub fn project_id_from_short_name(&self, short_name: &str) -> Option<ProjectId> {
        self.projects
            .iter()
            .filter(|&(_, val)| val.short_name == short_name)
            .map(|(key, _)| ProjectId::Id(*key))
            .next()
    }

    pub fn project_from_project_id(&self, project_id: ProjectId) -> Option<&Project> {
        if let ProjectId::Id(project_id) = project_id {
            self.projects.get(&project_id)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Creates a simple database, writes it to a file, loads the written file
    /// and checks that the contents are the same as the original data.
    fn write_read_db() {
        let file_name = Path::new("test_files/read_write_test.json");
        let mut checkpoint_db = CheckpointDb::new();

        let time_now = Utc::now().timestamp();

        let zro_id = checkpoint_db.add_project("Zeroeth", "zro").unwrap();
        checkpoint_db.add_project("First", "frs").unwrap();
        let scn_id = checkpoint_db.add_project("Second", "scn").unwrap();

        // Adding a project with a short name that already exists should not work.
        assert!(
            checkpoint_db.add_project("Duplicate", "scn").is_err(),
            "Adding a duplicate project didn't fail, but it should"
        );

        // Removing a project should work.
        {
            let time = time_now + 10;
            let message = "This checkpoint should have no projects";
            let rmv_id = checkpoint_db
                .add_project("This project should be removed", "rmv")
                .unwrap();
            checkpoint_db.add_checkpoint(time, message, rmv_id).unwrap();
            assert!(
                checkpoint_db.remove_project(rmv_id).is_ok(),
                "Could not remove a project"
            );
            assert_eq!(
                *checkpoint_db
                    .get_checkpoint(&CheckpointId::Timestamp(time))
                    .unwrap(),
                Checkpoint {
                    message: message.to_string(),
                    project_id: ProjectId::NoId,
                }
            );
        }

        checkpoint_db
            .add_checkpoint(time_now, "This checkpoint should be overwritten", zro_id)
            .unwrap();

        // Overwriting an existing checkpoint.
        checkpoint_db
            .add_checkpoint(time_now + 1, "This is a message", scn_id)
            .unwrap();

        // Adding and then removing an checkpoint.
        checkpoint_db
            .add_checkpoint(
                time_now + 2,
                "This checkpoint should be removed",
                ProjectId::NoId,
            )
            .unwrap();
        assert!(checkpoint_db
            .remove_checkpoint(&CheckpointId::Timestamp(time_now + 2))
            .is_some());

        assert!(checkpoint_db.write(&file_name).is_ok());

        let checkpoint_db_read = CheckpointDb::read(&file_name).unwrap();
        assert_eq!(checkpoint_db, checkpoint_db_read);
    }
}
