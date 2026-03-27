//! Tracing [`Layer`](tracing_subscriber::Layer) for `seraphim`

use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::Mutex;

use dashmap::DashMap;
use tracing::Event as TracingEvent;
use tracing::Subscriber;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Metadata, callsite::Identifier, subscriber::Interest};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

use crate::store::Store;
use crate::types::{Event, EventRef, Kind, Level, Value};

/// Tracing [`Layer`] that saves logs in a database and serves them over a
/// WebSocket server
#[derive(Debug, Clone)]
pub struct Seraphim {
    store: Arc<Mutex<Store>>,
    callsites: Arc<DashMap<Identifier, EventRef>>,
    disabled: Arc<DashMap<Identifier, EventRef>>,
    spans: Arc<DashMap<Id, (EventRef, bool, bool)>>,
    min_level: Level,
    package: String,
}

impl Seraphim {
    /// Creates a new [`Seraphim`] [`Layer`] from an instance of the storage
    /// engine
    pub fn new(store: Arc<Mutex<Store>>) -> Seraphim {
        Seraphim {
            store,
            callsites: Arc::new(DashMap::new()),
            disabled: Arc::new(DashMap::new()),
            spans: Arc::new(DashMap::new()),
            min_level: Level::Debug,
            package: "".into(),
        }
    }
}

impl<S: Subscriber> Layer<S> for Seraphim {
    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        let mut fields = metadata
            .fields()
            .iter()
            .map(|field| (field.index(), field.name().to_string()))
            .collect::<Vec<_>>();
        fields.sort_by_key(|(i, _)| *i);

        let event = Event::Callsite {
            name: metadata.name().to_string(),
            target: metadata.target().to_string(),
            level: metadata.level().clone().into(),
            file: metadata.file().map(ToOwned::to_owned),
            line: metadata.line(),
            module_path: metadata.module_path().map(ToOwned::to_owned),
            kind: if metadata.is_event() {
                Kind::Event
            } else if metadata.is_span() {
                Kind::Span
            } else {
                Kind::Hint
            },
            fields: fields.into_iter().map(|(_, field)| field).collect(),
        };

        let Ok(event_ref) = self.store.lock().unwrap().push(event) else {
            return Interest::always();
        };

        if Level::from(*metadata.level()) > self.min_level {
            self.disabled.insert(metadata.callsite(), event_ref);
            return Interest::always();
        }

        if !metadata.target().starts_with(&self.package) {
            self.disabled.insert(metadata.callsite(), event_ref);
            return Interest::always();
        }

        self.callsites.insert(metadata.callsite(), event_ref);

        Interest::always()
    }

    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let callsite = attrs.metadata().callsite();
        let Some(callsite) = self.callsites.get(&callsite) else {
            return;
        };
        let callsite = *callsite;

        let parent = if let Some(parent) = attrs.parent() {
            Some(parent.into_u64())
        } else if attrs.is_contextual() {
            ctx.current_span().id().map(Id::into_u64)
        } else if attrs.is_root() {
            None
        } else {
            return;
        };

        let mut visitor = IndexRecorder::new();
        attrs.record(&mut visitor);
        let attributes = visitor.into_values();

        let event = Event::Span {
            parent,
            callsite,
            attributes,
        };
        let Ok(event_ref) = self.store.lock().unwrap().push(event) else {
            return;
        };

        self.spans.insert(id.clone(), (event_ref, false, false));
    }

    fn on_record(&self, span: &Id, record: &Record<'_>, _ctx: Context<'_, S>) {
        let Some(span) = self.spans.get(span) else {
            return;
        };

        let mut visitor = NamedRecorder::new();
        record.record(&mut visitor);
        let values = visitor.into_values();

        let event = Event::Record {
            span: span.0,
            values,
        };

        self.store.lock().unwrap().push(event).ok();
    }

    fn on_follows_from(&self, span: &Id, follows: &Id, _ctx: Context<'_, S>) {
        let Some(span) = self.spans.get(span) else {
            return;
        };
        let Some(follows) = self.spans.get(follows) else {
            return;
        };
        let event = Event::FollowsFrom {
            span: span.0,
            follows: follows.0,
        };
        self.store.lock().unwrap().push(event).ok();
    }

    fn on_event(&self, event: &TracingEvent<'_>, ctx: Context<'_, S>) {
        let callsite = event.metadata().callsite();
        let Some(callsite) = self.callsites.get(&callsite) else {
            return;
        };
        let callsite = *callsite;

        let parent = if let Some(parent) = event.parent() {
            Some(parent.into_u64())
        } else if event.is_contextual() {
            ctx.current_span().id().map(Id::into_u64)
        } else if event.is_root() {
            None
        } else {
            return;
        };

        if let Some(mut span) = parent.and_then(|p| self.spans.get_mut(&Id::from_u64(p))) {
            if span.1 == false && span.2 == true {
                self.store
                    .lock()
                    .unwrap()
                    .push(Event::Enter { span: span.0 })
                    .ok();
                span.1 = true;
            } else if span.1 == true && span.2 == false {
                self.store
                    .lock()
                    .unwrap()
                    .push(Event::Exit { span: span.0 })
                    .ok();
                span.1 = false;
            }
        }

        let mut visitor = IndexRecorder::new();
        event.record(&mut visitor);
        let values = visitor.into_values();

        let event = Event::Event {
            parent,
            callsite,
            values,
        };
        self.store.lock().unwrap().push(event).ok();
    }

    fn on_enter(&self, id: &Id, _ctx: Context<'_, S>) {
        let Some(mut span) = self.spans.get_mut(id) else {
            return;
        };
        span.2 = true;
    }

    fn on_exit(&self, id: &Id, _ctx: Context<'_, S>) {
        let Some(mut span) = self.spans.get_mut(id) else {
            return;
        };
        span.2 = false;
    }

    fn on_close(&self, id: Id, _ctx: Context<'_, S>) {
        let Some((_id, span)) = self.spans.remove(&id) else {
            return;
        };
        self.store
            .lock()
            .unwrap()
            .push(Event::Close { span: span.0 })
            .ok();
    }

    fn on_id_change(&self, old: &Id, new: &Id, _ctx: Context<'_, S>) {
        if let Some((_old, event_ref)) = self.spans.remove(old) {
            self.spans.insert(new.clone(), event_ref);
        }
    }
}

#[derive(Debug, Default)]
struct IndexRecorder {
    values: Vec<(usize, Value)>,
}

impl IndexRecorder {
    fn new() -> IndexRecorder {
        IndexRecorder::default()
    }

    fn into_values(mut self) -> Vec<Value> {
        self.values.sort_by_key(|(i, _)| *i);
        self.values.into_iter().map(|(_, val)| val).collect()
    }
}

impl Visit for IndexRecorder {
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.values.push((field.index(), Value::F64(value)));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.values.push((field.index(), Value::I64(value)));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.values.push((field.index(), Value::U64(value)));
    }

    fn record_i128(&mut self, field: &Field, value: i128) {
        self.values.push((field.index(), Value::I128(value)));
    }

    fn record_u128(&mut self, field: &Field, value: u128) {
        self.values.push((field.index(), Value::U128(value)));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.values.push((field.index(), Value::Bool(value)));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.values
            .push((field.index(), Value::String(value.to_string())));
    }

    fn record_bytes(&mut self, field: &Field, value: &[u8]) {
        self.values
            .push((field.index(), Value::Bytes(value.to_vec())));
    }

    fn record_error(&mut self, field: &Field, mut value: &(dyn StdError + 'static)) {
        let mut messages = Vec::new();
        loop {
            messages.push(value.to_string());
            let Some(source) = value.source() else {
                break;
            };
            value = source;
        }
        self.values.push((field.index(), Value::Error(messages)));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        self.values
            .push((field.index(), Value::Debug(format!("{value:?}"))));
    }
}

#[derive(Debug, Default)]
struct NamedRecorder {
    names: Vec<(usize, String)>,
    values: Vec<(usize, Value)>,
}

impl NamedRecorder {
    fn new() -> NamedRecorder {
        NamedRecorder::default()
    }

    fn into_values(mut self) -> HashMap<String, Value> {
        self.values.sort_by_key(|(i, _)| *i);
        self.names.sort_by_key(|(i, _)| *i);
        self.values.dedup_by_key(|(i, _)| *i);
        self.names.dedup_by_key(|(i, _)| *i);
        self.values
            .into_iter()
            .zip(self.names)
            .map(|((_, val), (_, name))| (name, val))
            .collect()
    }
}

impl Visit for NamedRecorder {
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.names.push((field.index(), field.name().to_string()));
        self.values.push((field.index(), Value::F64(value)));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.names.push((field.index(), field.name().to_string()));
        self.values.push((field.index(), Value::I64(value)));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.names.push((field.index(), field.name().to_string()));
        self.values.push((field.index(), Value::U64(value)));
    }

    fn record_i128(&mut self, field: &Field, value: i128) {
        self.names.push((field.index(), field.name().to_string()));
        self.values.push((field.index(), Value::I128(value)));
    }

    fn record_u128(&mut self, field: &Field, value: u128) {
        self.names.push((field.index(), field.name().to_string()));
        self.values.push((field.index(), Value::U128(value)));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.names.push((field.index(), field.name().to_string()));
        self.values.push((field.index(), Value::Bool(value)));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.names.push((field.index(), field.name().to_string()));
        self.values
            .push((field.index(), Value::String(value.to_string())));
    }

    fn record_bytes(&mut self, field: &Field, value: &[u8]) {
        self.names.push((field.index(), field.name().to_string()));
        self.values
            .push((field.index(), Value::Bytes(value.to_vec())));
    }

    fn record_error(&mut self, field: &Field, mut value: &(dyn StdError + 'static)) {
        let mut messages = Vec::new();
        loop {
            messages.push(value.to_string());
            let Some(source) = value.source() else {
                break;
            };
            value = source;
        }
        self.names.push((field.index(), field.name().to_string()));
        self.values.push((field.index(), Value::Error(messages)));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        self.names.push((field.index(), field.name().to_string()));
        self.values
            .push((field.index(), Value::Debug(format!("{value:?}"))));
    }
}
