//! Networing for `seraphim` using [`std::net`]

use std::{
    io::{self, Read, Write},
    mem,
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread::spawn,
    time::Duration,
};

use postcard::{from_bytes, to_extend};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::Receiver;

use crate::{
    store::Store,
    types::{Event, EventRef},
};

pub fn serve(store: Arc<Mutex<Store>>, recv: Receiver<Event>, listener: TcpListener) {
    spawn(move || {
        while let Ok((conn, _peer)) = listener.accept() {
            let store = store.clone();
            let recv = recv.resubscribe();
            spawn(move || {
                if let Err(err) = handle_conn(conn, store, recv) {
                    eprintln!("Error while handling connection ({err:#})");
                }
            });
        }
    });
}

fn handle_conn(
    mut conn: TcpStream,
    store: Arc<Mutex<Store>>,
    mut recv: Receiver<Event>,
) -> io::Result<()> {
    conn.set_read_timeout(Some(Duration::from_millis(250)))?;
    let mut frame = Vec::new();
    loop {
        let mut len = [0; 4];
        match conn.read_exact(&mut len) {
            Ok(()) => {
                frame.resize(u32::from_be_bytes(len) as usize, 0);
                conn.read_exact(&mut frame)?;
                let request: Request = from_bytes(&frame)
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
                match request {
                    Request::Status => {
                        let end = store.lock().unwrap().len();
                        frame.clear();
                        let encoded =
                            to_extend(&Response::Status { end }, mem::take(&mut frame))
                                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
                        conn.write_all(&(encoded.len() as u32).to_be_bytes())?;
                        conn.write_all(&encoded)?;
                        frame = encoded;
                    }
                    Request::Read { start, stop } => {
                        let events = store.lock().unwrap().read(start..stop)?;
                        frame.clear();
                        let encoded =
                            to_extend(&Response::Read { start, events }, mem::take(&mut frame))
                                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
                        conn.write_all(&(encoded.len() as u32).to_be_bytes())?;
                        conn.write_all(&encoded)?;
                        frame = encoded;
                    }
                }
            }
            Err(err)
                if err.kind() == io::ErrorKind::TimedOut
                    || err.kind() == io::ErrorKind::WouldBlock =>
            {
                let mut events = Vec::new();
                while let Ok(event) = recv.try_recv() {
                    events.push(event);
                }
                if !events.is_empty() {
                    frame.clear();
                    let encoded = to_extend(
                        &Response::Update {
                            start: store.lock().unwrap().len() - 1,
                            events,
                        },
                        mem::take(&mut frame),
                    )
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
                    conn.write_all(&(encoded.len() as u32).to_be_bytes())?;
                    conn.write_all(&encoded)?;
                    frame = encoded;
                }
            }
            Err(err) => {
                return Err(err);
            }
        }
    }
}

/// Request which can be sent to the `seraphim` server to access traces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    Status,
    Read { start: EventRef, stop: EventRef },
}

/// Response from the `seraphim` server returning traces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    Status { end: EventRef },
    Read { start: EventRef, events: Vec<Event> },
    Update { start: EventRef, events: Vec<Event> },
}
