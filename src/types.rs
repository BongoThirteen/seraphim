//! Types for representing [`tracing`] log events

use std::{
    collections::HashMap,
    fmt::{self, Display, Formatter},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};
use tracing::Level as TracingLevel;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<TracingLevel> for Level {
    fn from(level: TracingLevel) -> Level {
        if level == TracingLevel::ERROR {
            Level::Error
        } else if level == TracingLevel::WARN {
            Level::Warn
        } else if level == TracingLevel::INFO {
            Level::Info
        } else if level == TracingLevel::DEBUG {
            Level::Debug
        } else {
            Level::Trace
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub enum Kind {
    Event,
    Span,
    Hint,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum Value {
    Bool(bool),
    Char(char),
    F32(f32),
    F64(f64),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    Isize(isize),
    String(String),
    Bytes(Vec<u8>),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    Usize(usize),
    Path(PathBuf),
    Error(Vec<String>),
    Debug(String),
}

impl Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Value::Bool(val) => write!(f, "{val}"),
            Value::Char(val) => write!(f, "{val}"),
            Value::F32(val) => write!(f, "{val}"),
            Value::F64(val) => write!(f, "{val}"),
            Value::I8(val) => write!(f, "{val}"),
            Value::I16(val) => write!(f, "{val}"),
            Value::I32(val) => write!(f, "{val}"),
            Value::I64(val) => write!(f, "{val}"),
            Value::I128(val) => write!(f, "{val}"),
            Value::Isize(val) => write!(f, "{val}"),
            Value::String(val) => write!(f, "{val}"),
            Value::Bytes(val) => write!(f, "{val:02X?}"),
            Value::U8(val) => write!(f, "{val}"),
            Value::U16(val) => write!(f, "{val}"),
            Value::U32(val) => write!(f, "{val}"),
            Value::U64(val) => write!(f, "{val}"),
            Value::U128(val) => write!(f, "{val}"),
            Value::Usize(val) => write!(f, "{val}"),
            Value::Path(val) => write!(f, "{}", val.display()),
            Value::Error(val) => write!(f, "{}", val[0]),
            Value::Debug(val) => write!(f, "{val}"),
        }
    }
}

pub type EventRef = u64;

/// An event received by the [`Seraphim`](super::Seraphim) [`Layer`](tracing_subscriber::Layer)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Event {
    /// Call to [`register_callsite`](Layer::register_callsite)
    Callsite {
        name: String,
        target: String,
        level: Level,
        file: Option<String>,
        line: Option<u32>,
        module_path: Option<String>,
        fields: Vec<String>,
        kind: Kind,
    },
    /// Call to [`on_new_span`](Layer::on_new_span)
    Span {
        parent: Option<EventRef>,
        callsite: EventRef,
        attributes: Vec<Value>,
    },
    /// Call to [`on_record`](Layer::on_record)
    Record {
        span: EventRef,
        values: HashMap<String, Value>,
    },
    /// Call to [`on_follows_from`](Layer::on_follows_from)
    FollowsFrom {
        span: EventRef,
        follows: EventRef,
    },
    /// Call to [`on_event`](Layer::on_event)
    Event {
        parent: Option<EventRef>,
        callsite: EventRef,
        values: Vec<Value>,
    },
    Enter {
        span: EventRef,
    },
    /// Call to [`on_exit`](Layer::on_exit)
    Exit {
        span: EventRef,
    },
    /// Call to [`on_close`](Layer::on_close)
    Close {
        span: EventRef,
    },
}
