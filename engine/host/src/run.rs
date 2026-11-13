//! The engine entry point: dispatch one request to the model it names.

use std::io::{self, BufRead, BufReader, Read, Write};

use serde_json::json;

use crate::inspect::{demo, describe, evaluate};
use crate::model::Model;
use crate::output::Output;
use crate::protocol::{Command, EngineError, Request};
use crate::solve::solve;

type Writer = Box<dyn Write>;
type Runner = fn(Request, Box<dyn BufRead + Send>, &mut Output<Writer>) -> Result<(), EngineError>;

/// One model the engine binary can run.
pub struct Entry {
    pub id: &'static str,
    pub name: &'static str,
    run: Runner,
}

/// Registers model `M` with the engine.
pub fn entry<M: Model>() -> Entry {
    Entry {
        id: M::id(),
        name: M::name(),
        run: dispatch::<M>,
    }
}

fn dispatch<M: Model>(
    request: Request,
    controls: Box<dyn BufRead + Send>,
    out: &mut Output<Writer>,
) -> Result<(), EngineError> {
    match request.command {
        Command::Describe => {
            describe::<M, _>(out);
            Ok(())
        }
        Command::Demo => demo::<M, _>(&request, out),
        Command::Evaluate => evaluate::<M, _>(request, out),
        Command::Solve => solve::<M, _>(request, controls, out),
    }
}

/// Runs the engine on the process's arguments, stdin and stdout, and returns
/// the exit code.
pub fn run(version: &str, models: &[Entry]) -> i32 {
    let version_flag = std::env::args().nth(1).as_deref() == Some("--version");
    run_io(
        version,
        models,
        version_flag,
        io::stdin(),
        Box::new(io::stdout()),
    )
}

/// Runs the engine on the given streams.
///
/// With `version_flag` it reports the engine version and its models. Otherwise
/// the first input line is the request; for `solve`, later lines are control
/// messages and the end of input cancels the solve.
pub fn run_io(
    version: &str,
    models: &[Entry],
    version_flag: bool,
    input: impl Read + Send + 'static,
    output: Writer,
) -> i32 {
    let mut out = Output::new(output);
    if version_flag {
        let listed: Vec<_> = models
            .iter()
            .map(|model| json!({ "id": model.id, "name": model.name }))
            .collect();
        out.event("version", json!({ "version": version, "models": listed }));
        return 0;
    }
    let mut input = BufReader::new(input);
    let mut line = String::new();
    if let Err(error) = input.read_line(&mut line) {
        out.error(&EngineError::new("io_error", error.to_string()));
        return 1;
    }
    let request: Request = match serde_json::from_str(&line) {
        Ok(request) => request,
        Err(error) => {
            out.error(&EngineError::new("invalid_request", error.to_string()));
            return 1;
        }
    };
    let Some(model) = models.iter().find(|model| model.id == request.model) else {
        let known: Vec<_> = models.iter().map(|model| model.id).collect();
        out.error(
            &EngineError::new(
                "unknown_model",
                format!("no model is named {:?}", request.model),
            )
            .with_detail(json!({ "models": known })),
        );
        return 1;
    };
    match (model.run)(request, Box::new(input), &mut out) {
        Ok(()) => 0,
        Err(error) => {
            out.error(&error);
            1
        }
    }
}
