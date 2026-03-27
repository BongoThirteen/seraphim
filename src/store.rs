//! Storage and serving of logs for `seraphim`

use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::ops::Range;
use std::path::Path;

use postcard::{from_bytes, to_stdvec};

use crate::types::{Event, EventRef};

/// An append-only database of events, either in-memory or backed by a file
#[derive(Debug)]
pub struct Store {
    /// Events from this session stored in memory
    session: Vec<Event>,
    /// Optionally, a file-backed database to save events to
    db: Option<Database>,
    #[cfg(feature = "net")]
    send: tokio::sync::broadcast::Sender<Event>,
}

impl Store {
    /// Create a new in-memory database for events
    ///
    /// This is ephemeral, as all events added to this database will be deleted
    /// when it is dropped.
    pub fn in_memory(#[cfg(feature = "net")] send: tokio::sync::broadcast::Sender<Event>) -> Store {
        Store {
            session: Vec::new(),
            db: None,
            #[cfg(feature = "net")]
            send,
        }
    }

    /// Create a new file-backed database for events
    ///
    /// Creates a new file if it doesn't exist, or opens the existing file
    pub fn open(
        path: impl AsRef<Path>,
        #[cfg(feature = "net")] send: tokio::sync::broadcast::Sender<Event>,
    ) -> io::Result<Store> {
        let db = Database::open(path)?;
        Ok(Store {
            session: Vec::new(),
            db: Some(db),
            #[cfg(feature = "net")]
            send,
        })
    }

    pub fn len(&self) -> u64 {
        if let Some(db) = &self.db {
            db.len()
        } else {
            self.session.len() as u64
        }
    }

    /// Add an event to this database
    ///
    /// This may or may not write to a file, so it returns [`io::Error`] if
    /// that fails.
    /// Writing to an in-memory database will never error.
    pub fn push(&mut self, event: Event) -> io::Result<EventRef> {
        let event_ref = self.len();
        if let Some(db) = &mut self.db {
            db.push(&event)?;
        };

        #[cfg(feature = "net")]
        {
            _ = self.send.send(event.clone());
        }

        self.session.push(event);

        Ok(event_ref)
    }

    pub fn read(&self, range: Range<EventRef>) -> io::Result<Vec<Event>> {
        if let Some(db) = &self.db {
            let session_start = self.len() - self.session.len() as u64;
            if range.start >= session_start {
                self.session
                    .get(
                        range.start as usize - session_start as usize
                            ..range.end as usize - session_start as usize,
                    )
                    .map(ToOwned::to_owned)
                    .ok_or(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "log entry index out of bound",
                    ))
            } else {
                db.read(range)
            }
        } else {
            let range = range.start as usize..range.end as usize;
            self.session
                .get(range)
                .map(ToOwned::to_owned)
                .ok_or(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "log entry index out of bounds",
                ))
        }
    }
}

/// File-backed event database
///
/// Internally, events are just serialized and appended to the file.
#[derive(Debug)]
pub struct Database {
    file: BufWriter<File>,
    len: u64,
}

impl Database {
    /// Create a new file-backed event database
    ///
    /// This creates a new file if it doesn't already exist.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Database> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;
        let written = file.metadata()?.len();
        let len = if written == 0 {
            file.write_all(&[0; 8])?;
            0
        } else {
            file.seek(SeekFrom::Start(0))?;
            let mut len = [0; 8];
            file.read_exact(&mut len)?;
            file.seek(SeekFrom::End(0))?;
            u64::from_be_bytes(len)
        };
        let db = Database {
            file: BufWriter::new(file),
            len,
        };
        Ok(db)
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    /// Add an event to this file-backed database
    pub fn push(&mut self, event: &Event) -> io::Result<()> {
        let bytes =
            to_stdvec(event).map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

        if bytes.len() > u32::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "max event length of u32::MAX bytes exceeded",
            ));
        }

        self.file.write_all(&self.len.to_be_bytes())?;
        self.file.write_all(&(bytes.len() as u32).to_be_bytes())?;
        self.file.write_all(&bytes)?;
        self.len += 1;

        Ok(())
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }

    pub fn read(&self, range: Range<EventRef>) -> io::Result<Vec<Event>> {
        let mut file = BufReader::new(self.file.get_ref());

        file.seek(SeekFrom::Start(8))?;
        let mut index = 0;
        let mut events = Vec::with_capacity((range.end - range.start).try_into().unwrap());
        while index < range.end {
            let mut len = [0; 4];
            file.read_exact(&mut len)?;
            if index < range.start {
                file.seek_relative(u32::from_be_bytes(len) as i64)?;
            } else {
                let mut buf = vec![0; u32::from_be_bytes(len) as usize];
                file.read_exact(&mut buf)?;
                let event = from_bytes(&buf)
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
                events.push(event);
            }
            index += 1;
        }
        Ok(events)
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        if self.file.seek(SeekFrom::Start(0)).is_err() {
            return;
        }
        self.file.write_all(&self.len.to_be_bytes()).ok();
    }
}
