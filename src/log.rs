#![allow(dead_code)]

use console::Style;
use linked_hash_map::LinkedHashMap;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    ops::Range,
};
use tracing::{Id, Level};
use tracing_subscriber::Layer;

use std::sync::{Arc, Mutex};

const TRACE_TEST_SPAN: &str = "test";
const IGNORE_LIST: &[&str] = &[];

/// An in-memory logger that lets us view particular
/// spans of the logs, and understands minidump-stackwalk's
/// span format for threads/frames during stackwalking.
#[derive(Default, Debug, Clone)]
pub struct MapLogger {
    state: Arc<Mutex<MapLoggerInner>>,
}

type SpanId = u64;

#[derive(Default, Debug, Clone)]
struct MapLoggerInner {
    root_span: SpanEntry,
    sub_spans: LinkedHashMap<SpanId, SpanEntry>,

    last_query: Option<Query>,
    cur_string: Option<Arc<String>>,

    test_spans: HashSet<SpanId>,
    live_spans: HashMap<Id, SpanId>,
    next_span_id: SpanId,
}

#[derive(Default, Debug, Clone)]
struct SpanEntry {
    destroyed: bool,
    name: String,
    fields: BTreeMap<String, String>,
    events: Vec<EventEntry>,
}

#[derive(Debug, Clone)]
enum EventEntry {
    Span(SpanId),
    Message(MessageEntry),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct MessageEntry {
    level: Level,
    fields: BTreeMap<String, String>,
    target: String,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Query {
    All,
    Span(SpanId),
}

impl MapLogger {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&self) {
        let mut log = self.state.lock().unwrap();
        let ids = log.sub_spans.keys().cloned().collect::<Vec<_>>();
        for id in ids {
            let span = log.sub_spans.get_mut(&id).unwrap();
            if !span.destroyed {
                span.events.clear();
                continue;
            }
            log.sub_spans.remove(&id);
        }
        log.root_span.events.clear();
        log.cur_string = None;
    }

    fn print_span_if_test(&self, span_id: &Id) {
        let span = {
            let log = self.state.lock().unwrap();
            let Some(span) = log.live_spans.get(span_id) else {
                return;
            };
            if !log.test_spans.contains(span) {
                return;
            }
            *span
        };
        eprintln!("{}", self.string_for_span(span));
    }

    pub fn string_for_span(&self, span: SpanId) -> Arc<String> {
        self.string_query(Query::Span(span))
    }
    /*
       pub fn string_for_all(&self) -> Arc<String> {
           self.string_query(Query::All)
       }

       pub fn string_for_thread(&self, thread_idx: usize) -> Arc<String> {
           let thread = self
               .state
               .lock()
               .unwrap()
               .thread_spans
               .get(&thread_idx)
               .cloned();

           if let Some(thread) = thread {
               self.string_query(Query::Thread(thread))
           } else {
               Arc::new(String::from("thread whoops!"))
               // self.string_query(Query::All)
           }
       }

       pub fn string_for_frame(&self, thread_idx: usize, frame_idx: usize) -> Arc<String> {
           let thread = self
               .state
               .lock()
               .unwrap()
               .thread_spans
               .get(&thread_idx)
               .cloned();

           let frame = self
               .state
               .lock()
               .unwrap()
               .frame_spans
               .get(&(thread_idx, frame_idx))
               .cloned();

           if let (Some(thread), Some(frame)) = (thread, frame) {
               self.string_query(Query::Frame(thread, frame))
           } else {
               Arc::new(String::from("frame whoops!"))
               // self.string_query(Query::All)
           }
       }
    */
    fn string_query(&self, query: Query) -> Arc<String> {
        use std::fmt::Write;

        fn print_indent(output: &mut String, depth: usize) {
            write!(output, "{:indent$}", "", indent = depth * 4).unwrap();
        }
        fn print_span_recursive(
            output: &mut String,
            sub_spans: &LinkedHashMap<SpanId, SpanEntry>,
            depth: usize,
            span: &SpanEntry,
            range: Option<Range<usize>>,
        ) {
            if !span.name.is_empty() {
                print_indent(output, depth);
                let style = Style::new().blue();
                write!(output, "{}", style.apply_to(&span.name)).unwrap();
                for (key, val) in &span.fields {
                    if key == "id" {
                        write!(output, " {}", style.apply_to(val)).unwrap();
                    } else {
                        write!(output, "{key}: {val}").unwrap();
                    }
                }
                writeln!(output).unwrap();
            }

            let event_range = if let Some(range) = range {
                &span.events[range]
            } else {
                &span.events[..]
            };
            for event in event_range {
                match event {
                    EventEntry::Message(event) => {
                        if event.fields.contains_key("message") {
                            print_indent(output, depth + 1);
                            print_event(output, event);
                        }
                    }
                    EventEntry::Span(sub_span) => {
                        print_span_recursive(
                            output,
                            sub_spans,
                            depth + 1,
                            &sub_spans[sub_span],
                            None,
                        );
                    }
                }
            }
        }

        let mut log = self.state.lock().unwrap();
        if Some(query) == log.last_query {
            if let Some(string) = &log.cur_string {
                return string.clone();
            }
        }
        log.last_query = Some(query);

        let mut output = String::new();

        let (span_to_print, range) = match query {
            Query::All => (&log.root_span, None),
            Query::Span(span_id) => (&log.sub_spans[&span_id], None),
        };

        print_span_recursive(&mut output, &log.sub_spans, 0, span_to_print, range);

        let result = Arc::new(output);
        log.cur_string = Some(result.clone());
        result
    }
}

fn immediate_event(event: &MessageEntry) {
    let mut output = String::new();
    print_event(&mut output, event);
    eprintln!("{}", output);
}

fn print_event(output: &mut String, event: &MessageEntry) {
    use std::fmt::Write;
    if let Some(message) = event.fields.get("message") {
        let style = match event.level {
            Level::ERROR => Style::new().red(),
            Level::WARN => Style::new().yellow(),
            Level::INFO => Style::new(),
            Level::DEBUG => Style::new().blue(),
            Level::TRACE => Style::new().green(),
        };
        // writeln!(output, "[{:5}] {}", event.level, message).unwrap();
        writeln!(output, "{}", style.apply_to(message)).unwrap();
    }
}

impl<S> Layer<S> for MapLogger
where
    S: tracing::Subscriber,
    S: for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let target = event.metadata().target();
        if IGNORE_LIST.iter().any(|module| target.starts_with(module)) {
            return;
        }
        let mut log = self.state.lock().unwrap();
        // Invalidate any cached log printout
        log.cur_string = None;

        // Grab the parent span (or the dummy root span)
        let (cur_span, is_root) = if let Some(span) = ctx.event_span(event) {
            let span_id = log.live_spans[&span.id()];
            (log.sub_spans.get_mut(&span_id).unwrap(), false)
        } else {
            (&mut log.root_span, true)
        };

        // Grab the fields
        let mut fields = BTreeMap::new();
        let mut visitor = MapVisitor(&mut fields);
        event.record(&mut visitor);

        // Store the message in the span
        let event = MessageEntry {
            level: *event.metadata().level(),
            fields,
            target: target.to_owned(),
        };
        if is_root {
            immediate_event(&event);
        }
        cur_span.events.push(EventEntry::Message(event));
    }

    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // let target = attrs.metadata().target();
        let mut log = self.state.lock().unwrap();
        // Create a new persistent id for this span, `tracing` may recycle its ids
        let new_span_id = log.next_span_id;
        log.next_span_id += 1;
        log.live_spans.insert(id.clone(), new_span_id);

        // Get the parent span (or dummy root span)
        let span = ctx.span(id).unwrap();
        let parent_span = if let Some(parent) = span.parent() {
            let parent_span_id = log.live_spans[&parent.id()];
            log.sub_spans.get_mut(&parent_span_id).unwrap()
        } else {
            &mut log.root_span
        };

        // Store the span at this point in the parent spans' messages,
        // so when we print out the parent span, this whole span will
        // print out "atomically" at this precise point in the log stream
        // which basically reconstitutes the logs of a sequential execution!
        parent_span.events.push(EventEntry::Span(new_span_id));

        // The actual span, with some info TBD
        let mut new_entry = SpanEntry {
            destroyed: false,
            name: span.name().to_owned(),
            fields: BTreeMap::new(),
            events: Vec::new(),
        };

        // Collect up fields for the span, and detect if it's a thread/frame span
        let mut visitor = SpanVisitor(&mut new_entry);
        attrs.record(&mut visitor);

        if span.name() == TRACE_TEST_SPAN {
            log.test_spans.insert(new_span_id);
        }

        log.sub_spans.insert(new_span_id, new_entry);
    }

    fn on_close(&self, id: Id, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        // Mark the span as GC-able and remove it from the live mappings,
        // as tracing may now recycle the id for future spans!
        self.print_span_if_test(&id);
        let mut log = self.state.lock().unwrap();
        let Some(&span_id) = log.live_spans.get(&id) else {
            // Skipped span, ignore
            return;
        };
        log.sub_spans.get_mut(&span_id).unwrap().destroyed = true;
        log.live_spans.remove(&id);
    }

    fn on_record(
        &self,
        id: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut log = self.state.lock().unwrap();

        // Update fields... idk we don't really need/use this but sure whatever
        let mut new_fields = BTreeMap::new();
        let mut visitor = MapVisitor(&mut new_fields);
        values.record(&mut visitor);

        let span_id = log.live_spans[id];
        log.sub_spans
            .get_mut(&span_id)
            .unwrap()
            .fields
            .append(&mut new_fields);
    }
}

/// Same as MapVisitor but grabs the special `idx: u64` field
struct SpanVisitor<'a>(&'a mut SpanEntry);

impl<'a> tracing::field::Visit for SpanVisitor<'a> {
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.0.fields.insert(field.to_string(), value.to_string());
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.0.fields.insert(field.to_string(), value.to_string());
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.0.fields.insert(field.to_string(), value.to_string());
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.0.fields.insert(field.to_string(), value.to_string());
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.fields.insert(field.to_string(), value.to_string());
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.0.fields.insert(field.to_string(), value.to_string());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0
            .fields
            .insert(field.to_string(), format!("{value:?}"));
    }
}

/// Super boring generic field slurping
struct MapVisitor<'a>(&'a mut BTreeMap<String, String>);

impl<'a> tracing::field::Visit for MapVisitor<'a> {
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.0.insert(field.to_string(), value.to_string());
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.0.insert(field.to_string(), value.to_string());
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.0.insert(field.to_string(), value.to_string());
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.0.insert(field.to_string(), value.to_string());
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.insert(field.to_string(), value.to_string());
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.0.insert(field.to_string(), value.to_string());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.insert(field.to_string(), format!("{value:?}"));
    }
}
