//! The agent's "brain": turns system telemetry (game session detected, process
//! counts, memory pressure) into a throttle policy. Three tiers, always in this
//! order, always degrading gracefully:
//!
//! 1. Local model via Ollama (llama3.2:1b) — offline, low-latency, no data
//!    leaves the machine. Used for routine telemetry classification.
//! 2. Cloud escalation (Anthropic API) — only when ANTHROPIC_API_KEY is set,
//!    for reasoning the local model can't handle. Inert without the key.
//! 3. Rule-based fallback — deterministic, dependency-free, and the *floor*:
//!    any LLM failure (daemon can't reach Ollama, malformed JSON, timeout)
//!    lands here, so throttling never depends on an LLM being healthy.
//!
//! Blast radius is bounded by design: whatever any tier says, the output is a
//! clamped cpu.weight for the agent's own cgroup — the LLM cannot name PIDs,
//! kill anything, or touch the ProtectedSet-guarded paths at all.

mod cloud;
mod ollama;
mod orchestrator;
mod policy;

pub use cloud::CloudClient;
pub use ollama::OllamaClient;
pub use orchestrator::Orchestrator;
pub use policy::{Policy, PolicySource, TelemetryContext};

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("io error talking to local model: {0}")]
    Io(#[from] std::io::Error),

    #[error("local model returned malformed response: {0}")]
    MalformedResponse(String),

    #[error("cloud API error: {0}")]
    Cloud(String),

    #[error("no cloud API key configured")]
    NoCloudKey,

    #[error("timed out waiting for model")]
    Timeout,
}
