use std::time::Duration;

use serde::Deserialize;
use tracing::debug;

use crate::policy::{build_prompt, ModelVerdict, Policy, PolicySource, TelemetryContext};
use crate::LlmError;

/// Cloud escalation via the Anthropic Messages API. Model choice is deliberate:
/// `claude-haiku-4-5` is the cheapest/fastest tier, which is the right call for
/// this workload — it's a bounded classification (emit one small JSON object),
/// not open-ended reasoning, and it sits in the latency-sensitive path of a
/// game launch. Escalating to an Opus/Sonnet tier here would cost more and add
/// latency for no quality gain on a task this constrained.
///
/// This tier is entirely optional: constructed only when ANTHROPIC_API_KEY is
/// present, and the daemon works fully without it (local + rule-based cover the
/// offline case, which is the norm for a living-room gaming box).
pub struct CloudClient {
    api_key: String,
    model: String,
    timeout: Duration,
}

const MESSAGES_ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    text: String,
}

impl CloudClient {
    /// Returns None (not an error) when no API key is configured — callers treat
    /// the cloud tier as simply unavailable, never as a failure to log loudly.
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY").ok()?;
        if api_key.trim().is_empty() {
            return None;
        }
        Some(Self {
            api_key,
            model: "claude-haiku-4-5".to_string(),
            timeout: Duration::from_secs(10),
        })
    }

    pub fn classify(&self, ctx: &TelemetryContext) -> Result<Policy, LlmError> {
        let body = serde_json::json!({
            "model": self.model,
            // Small cap: the reply is a one-line JSON object. This is a
            // classification, not a generation, so there's no reason to leave
            // room for a long response.
            "max_tokens": 256,
            "messages": [{ "role": "user", "content": build_prompt(ctx) }],
        });

        let resp = ureq::post(MESSAGES_ENDPOINT)
            .set("x-api-key", &self.api_key)
            .set("anthropic-version", ANTHROPIC_VERSION)
            .set("content-type", "application/json")
            .timeout(self.timeout)
            .send_json(body)
            .map_err(|e| LlmError::Cloud(e.to_string()))?;

        let parsed: MessagesResponse = resp
            .into_json()
            .map_err(|e| LlmError::Cloud(e.to_string()))?;

        let text = parsed
            .content
            .iter()
            .find(|b| b.block_type == "text")
            .map(|b| b.text.as_str())
            .ok_or_else(|| LlmError::Cloud("no text block in response".into()))?;

        debug!(raw = %text, "cloud verdict");

        let verdict: ModelVerdict = serde_json::from_str(text.trim())
            .map_err(|e| LlmError::Cloud(format!("unparseable verdict: {e}")))?;

        Ok(Policy {
            cpu_weight: verdict.cpu_weight,
            rationale: verdict.rationale,
            source: PolicySource::CloudModel,
        }
        .clamped())
    }
}
