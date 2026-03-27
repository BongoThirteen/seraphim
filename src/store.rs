//! Storage and serving of logs for `seraphim`

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::Path;

use postcard::to_stdvec;

use crate::types::{Event, EventRef};

/// An append-only database of events, either in-memory or backed by a file
#[derive(Debug)]
pub struct Store {
    /// Events from this session stored in memory
    session: VecDeque<Event>,
    /// Optionally, a file-backed database to save events to
    db: Option<Database>,
    /// Number of bytes written in total, used to identify events
    written: u64,
}

impl Store {
    /// Create a new in-memory database for events
    ///
    /// This is ephemeral, as all events added to this database will be deleted
    /// when it is dropped.
    pub fn in_memory() -> Store {
        Store {
            session: VecDeque::new(),
            db: None,
            written: 0,
        }
    }

    /// Create a new file-backed database for events
    ///
    /// Creates a new file if it doesn't exist, or opens the existing file
    pub fn open(path: impl AsRef<Path>) -> io::Result<Store> {
        let (db, written) = Database::open(path)?;
        Ok(Store {
            session: VecDeque::new(),
            db: Some(db),
            written,
        })
    }

    /// Add an event to this database
    ///
    /// This may or may not write to a file, so it returns [`io::Error`] if
    /// that fails.
    /// Writing to an in-memory database will never error.
    pub fn push(&mut self, event: Event) -> io::Result<EventRef> {
        let event_ref = self.written;
        if let Some(db) = &mut self.db {
            self.written += db.push(&event)? as u64;
        } else {
            self.written += 1;
        };

        self.session.push_back(event);

        Ok(event_ref)
    }
}

/// File-backed event database
///
/// Internally, events are just serialized and appended to the file.
#[derive(Debug)]
pub struct Database {
    file: BufWriter<File>,
}

impl Database {
    /// Create a new file-backed event database
    ///
    /// This creates a new file if it doesn't already exist.
    pub fn open(path: impl AsRef<Path>) -> io::Result<(Database, u64)> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;
        let written = file.metadata()?.len();
        let db = Database {
            file: BufWriter::new(file),
        };
        Ok((db, written))
    }

    /// Add an event to this file-backed database
    pub fn push(&mut self, event: &Event) -> io::Result<u32> {
        let bytes =
            to_stdvec(event).map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

        if bytes.len() > u32::MAX as usize - 4 {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "max event length of u32::MAX bytes exceeded",
            ));
        }

        self.file.write_all(&(bytes.len() as u32).to_be_bytes())?;
        self.file.write_all(&bytes)?;

        Ok(bytes.len() as u32 + 4)
    }
}
