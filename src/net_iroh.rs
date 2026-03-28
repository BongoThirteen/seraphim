//! Networking for `seraphim` using [`iroh`]

use std::{
    io, mem,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_util::sink::SinkExt;
use iroh::{
    endpoint::{Connection, VarInt},
    protocol::{AcceptError, ProtocolHandler},
};
use postcard::{from_bytes, to_extend};
use serde::{Deserialize, Serialize};
use tokio::{
    select,
    sync::broadcast::{Receiver, error::RecvError},
    time::{MissedTickBehavior, interval},
};
use tokio_stream::StreamExt;
use tokio_util::{
    bytes::{Buf, BufMut, BytesMut},
    codec::{Decoder, Encoder, FramedRead, FramedWrite},
};

use crate::{
    store::Store,
    types::{Event, EventRef},
};

/// QUIC ALPN for `seraphim`
pub const ALPN: &[u8] = b"seraphim/0";

/// `iroh` [`ProtocolHandler`] instance to run a `seraphim` server
#[derive(Debug)]
pub struct SeraphimProtocol {
    store: Arc<Mutex<Store>>,
    recv: Receiver<Event>,
}

impl SeraphimProtocol {
    pub fn new(store: Arc<Mutex<Store>>, recv: Receiver<Event>) -> SeraphimProtocol {
        SeraphimProtocol { store, recv }
    }
}

impl ProtocolHandler for SeraphimProtocol {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        let mut event_recv = self.recv.resubscribe();
        let (send, recv) = connection.accept_bi().await?;
        let (mut send, mut recv) = (
            FramedWrite::new(send, AcceptProtocol),
            FramedRead::new(recv, AcceptProtocol),
        );

        let mut update_buf = Vec::new();
        let mut update_clock = interval(Duration::from_millis(100));
        update_clock.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            select! {
                request = recv.next() => {
                    let Some(request) = request else {
                        break;
                    };
                    match request? {
                        Request::Status => {
                            let end = self.store.lock().unwrap().len();
                            send.send(&Response::Status { end }).await?;
                        }
                        Request::Read { start, stop } => {
                            let events = self.store.lock().unwrap().read(start..stop)?;
                            send.send(&Response::Read { start, events }).await?;
                        }
                    }
                }
                received = event_recv.recv() => {
                    let Ok(event) = received else {
                        if received == Err(RecvError::Closed) {
                            connection.close(VarInt::from_u32(1), b"logging channel closed");
                            break;
                        }
                        continue;
                    };
                    update_buf.push(event);
                    while let Ok(event) = event_recv.try_recv() {
                        update_buf.push(event);
                    }
                }
                _ = update_clock.tick() => {
                    if !update_buf.is_empty() {
                        let start = self.store.lock().unwrap().len();
                        send.send(&Response::Update { start, events: update_buf.drain(..).collect() }).await?;
                    }
                }
            }
        }

        Ok(())
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

struct AcceptProtocol;

impl Decoder for AcceptProtocol {
    type Item = Request;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 4 {
            src.reserve(4 - src.len());
            return Ok(None);
        }

        let frame_len = u32::from_be_bytes(src[0..4].try_into().unwrap());

        if src.len() < 4 + frame_len as usize {
            src.reserve(frame_len as usize - src.len());
            return Ok(None);
        }

        let frame = from_bytes(&src[4..4 + frame_len as usize])
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        if src.len() < frame_len as usize + 8 {
            src.reserve(frame_len as usize + 8 - src.len());
        }
        src.advance(4 + frame_len as usize);

        Ok(Some(frame))
    }
}

impl Encoder<&Response> for AcceptProtocol {
    type Error = io::Error;

    fn encode(&mut self, item: &Response, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.put_bytes(0, 4);
        let frame = to_extend(item, mem::take(dst))
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        *dst = frame;
        let frame_len = dst.len() - 4;
        if frame_len > u32::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "response would exceed u32::MAX bytes",
            ));
        }
        dst[0..4].copy_from_slice(&(frame_len as u32).to_be_bytes());

        Ok(())
    }
}

/// Network codec for communication between the `seraphim` client and server,
/// in the form of an implementation of [`Encoder`] and [`Decoder`]
pub struct ClientProtocol;

impl Decoder for ClientProtocol {
    type Item = Response;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 4 {
            src.reserve(4 - src.len());
            return Ok(None);
        }

        let frame_len = u32::from_be_bytes(src[0..4].try_into().unwrap());

        if src.len() < 4 + frame_len as usize {
            src.reserve(frame_len as usize + 4 - src.len());
            return Ok(None);
        }

        let frame = from_bytes(&src[4..4 + frame_len as usize])
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        if src.len() < frame_len as usize + 8 {
            src.reserve(frame_len as usize + 8 - src.len());
        }
        src.advance(4 + frame_len as usize);

        Ok(Some(frame))
    }
}

impl Encoder<&Request> for ClientProtocol {
    type Error = io::Error;

    fn encode(&mut self, item: &Request, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.put_bytes(0, 4);
        let frame = to_extend(item, mem::take(dst))
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        *dst = frame;
        let frame_len = dst.len() - 4;
        if frame_len > u32::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "response would exceed u32::MAX bytes",
            ));
        }
        dst[0..4].copy_from_slice(&(frame_len as u32).to_be_bytes());

        Ok(())
    }
}
