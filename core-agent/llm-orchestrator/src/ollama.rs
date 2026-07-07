use std::time::Duration;

use serde::Deserialize;
use tracing::debug;

use crate::policy::{build_prompt, ModelVerdict, Policy, PolicySource, TelemetryContext};
use crate::LlmError;

/// Talks to a local Ollama server (default 127.0.0.1:11434). Uses the
/// /api/generate endpoint with format=json and stream=false so we get a single
/// JSON body back and can parse the verdict directly.
pub struct OllamaClient {
    base_url: String,
    model: String,
    timeout: Duration,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

impl Default for OllamaClient {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:11434".to_string(),
            model: "llama3.2:1b".to_string(),
            // Local 1B model on CPU: a few seconds is normal, but the daemon
            // can't stall its poll loop, so cap it and fall back on timeout.
            timeout: Duration::from_secs(8),
        }
    }
}

impl OllamaClient {
    pub fn with_model(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            ..Default::default()
        }
    }

    pub fn classify(&self, ctx: &TelemetryContext) -> Result<Policy, LlmError> {
        let body = serde_json::json!({
            "model": self.model,
            "prompt": build_prompt(ctx),
            "format": "json",
            "stream": false,
        });

        let resp = ureq::post(&format!("{}/api/generate", self.base_url))
            .timeout(self.timeout)
            .send_json(body)
            .map_err(|e| match e {
                ureq::Error::Transport(t)
                    if matches!(t.kind(), ureq::ErrorKind::Io) =>
                {
                    LlmError::Timeout
                }
                other => LlmError::MalformedResponse(other.to_string()),
            })?;

        let parsed: OllamaResponse = resp
            .into_json()
            .map_err(|e| LlmError::MalformedResponse(e.to_string()))?;

        debug!(raw = %parsed.response, "ollama verdict");

        let verdict: ModelVerdict = serde_json::from_str(&parsed.response)
            .map_err(|e| LlmError::MalformedResponse(e.to_string()))?;

        Ok(Policy {
            cpu_weight: verdict.cpu_weight,
            rationale: verdict.rationale,
            source: PolicySource::LocalModel,
        }
        .clamped())
    }
}
