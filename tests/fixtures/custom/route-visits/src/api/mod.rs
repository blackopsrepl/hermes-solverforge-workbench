// The job, analysis, and telemetry DTOs serve library callers here; the CLI
// shell has no HTTP routes that would otherwise consume them.
pub mod dto;
pub mod telemetry;

pub use dto::PlanDto;
pub use telemetry::{CandidateTraceDto, TelemetryDto};
