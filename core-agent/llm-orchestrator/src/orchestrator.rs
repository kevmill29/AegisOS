use tracing::{info, warn};

use crate::cloud::CloudClient;
use crate::ollama::OllamaClient;
use crate::policy::{Policy, TelemetryContext};

/// Wires the three tiers together with the degradation order that is the whole
/// point of the hybrid design: try local first (fast, offline, private), fall
/// through to cloud only if local fails *and* a key exists, and land on the
/// rule-based floor if both are unavailable. The floor never fails, so
/// `decide` is infallible by construction — the daemon always gets a policy.
pub struct Orchestrator {
    local: OllamaClient,
    cloud: Option<CloudClient>,
}

impl Orchestrator {
    pub fn new() -> Self {
        let cloud = CloudClient::from_env();
        info!(
            cloud_available = cloud.is_some(),
            "LLM orchestrator initialized"
        );
        Self {
            local: OllamaClient::default(),
            cloud,
        }
    }

    /// Infallible: always returns a usable policy. Which tier produced it is on
    /// `policy.source` for auditability.
    pub fn decide(&self, ctx: &TelemetryContext) -> Policy {
        match self.local.classify(ctx) {
            Ok(policy) => return policy,
            Err(e) => warn!(error = %e, "local model failed, trying next tier"),
        }

        if let Some(cloud) = &self.cloud {
            match cloud.classify(ctx) {
                Ok(policy) => return policy,
                Err(e) => warn!(error = %e, "cloud model failed, falling back to rules"),
            }
        }

        Policy::rule_based_default(ctx)
    }
}

impl Default for Orchestrator {
    fn default() -> Self {
        Self::new()
    }
}
