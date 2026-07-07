//! Live demo of the orchestrator: feeds a synthetic "game just launched"
//! telemetry snapshot through the real tier chain and prints the policy plus
//! which tier produced it. Requires Ollama running with llama3.2:1b for the
//! local tier; without it, watch it degrade to rule-based.
use llm_orchestrator::{Orchestrator, TelemetryContext};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let ctx = TelemetryContext {
        game_name: Some("EldenRing.exe".to_string()),
        app_id: Some("1245620".to_string()),
        process_count: 342,
        total_memory_mb: 16000,
        available_memory_mb: 4200,
    };

    let orchestrator = Orchestrator::new();
    let policy = orchestrator.decide(&ctx);

    println!("--- policy decision ---");
    println!("source:     {:?}", policy.source);
    println!("cpu_weight: {}", policy.cpu_weight);
    println!("rationale:  {}", policy.rationale);
}
