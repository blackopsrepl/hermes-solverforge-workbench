//! JSON Lines written to stdout, one event per line, flushed as written.
//!
//! A model crate built with SolverForge's `console` or `verbose-logging`
//! feature prints its banner and solve summary to stdout from solver threads.
//! The engine therefore never holds the stdout lock between events, and writes
//! each event as one whole line, so readers keep the lines that parse as an
//! object with an `event` key and skip the rest.

use std::io::Write;

use serde_json::{json, Value};

use crate::protocol::EngineError;

/// The engine's only writer to stdout.
pub struct Output<W: Write> {
    writer: W,
}

impl<W: Write> Output<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Writes one event: `{"event": <name>, ...fields}`.
    pub fn event(&mut self, name: &str, fields: Value) {
        let mut line = json!({ "event": name });
        if let (Some(target), Value::Object(source)) = (line.as_object_mut(), fields) {
            target.extend(source);
        }
        let mut text = line.to_string();
        text.push('\n');
        // A closed stdout means the caller went away; there is no one to tell.
        let _ = self.writer.write_all(text.as_bytes());
        let _ = self.writer.flush();
    }

    pub fn error(&mut self, error: &EngineError) {
        self.event("error", json!({ "error": error }));
    }
}
