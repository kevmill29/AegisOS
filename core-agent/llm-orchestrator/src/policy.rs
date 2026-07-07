use serde::{Deserialize, Serialize};

/// What the daemon knows at decision time. Kept deliberately small and
/// pre-digested: the model classifies a summary, it does not get raw process
/// lists to freelance over.
#[derive(Debug, Clone, Serialize)]
pub struct TelemetryContext {
    pub game_name: Option<String>,
    pub app_id: Option<String>,
    pub process_count: usize,
    pub total_memory_mb: u64,
    pub available_memory_mb: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PolicySource {
    LocalModel,
    CloudModel,
    RuleBased,
}

/// The only thing any tier is allowed to produce: a cpu.weight for the agent's
/// own background cgroup, plus a human-auditable rationale.
#[derive(Debug, Clone, Serialize)]
pub struct Policy {
    pub cpu_weight: u32,
    pub rationale: String,
    pub source: PolicySource,
}

/// Weight bounds enforced on every policy regardless of source. Floor of 5
/// (not 1) for the same reason SelfThrottle uses 10 as its default: the agent
/// must stay schedulable enough to notice the game exiting.
pub const MIN_WEIGHT: u32 = 5;
pub const MAX_WEIGHT: u32 = 100;

impl Policy {
    pub fn clamped(mut self) -> Self {
        self.cpu_weight = self.cpu_weight.clamp(MIN_WEIGHT, MAX_WEIGHT);
        self
    }

    pub fn rule_based_default(ctx: &TelemetryContext) -> Self {
        // Deterministic heuristic: throttle harder when memory is tight, since
        // that's when background reclaim hurts a game most.
        let memory_pressure = if ctx.total_memory_mb > 0 {
            1.0 - (ctx.available_memory_mb as f64 / ctx.total_memory_mb as f64)
        } else {
            0.5
        };
        let cpu_weight = if memory_pressure > 0.85 { MIN_WEIGHT } else { 10 };
        Policy {
            cpu_weight,
            rationale: format!(
                "rule-based: memory pressure {:.0}%, weight {}",
                memory_pressure * 100.0,
                cpu_weight
            ),
            source: PolicySource::RuleBased,
        }
    }
}

/// The strict-JSON shape both model tiers are prompted to emit.
#[derive(Debug, Deserialize)]
pub struct ModelVerdict {
    pub cpu_weight: u32,
    pub rationale: String,
}

pub fn build_prompt(ctx: &TelemetryContext) -> String {
    format!(
        "You are the resource-allocation brain of a gaming OS. A game just launched \
         and the background agent must pick how hard to throttle itself.\n\
         Telemetry: game={:?} app_id={:?} processes={} memory_total_mb={} memory_available_mb={}\n\
         Reply with ONLY a JSON object: {{\"cpu_weight\": <integer 5-100, lower = more throttled, \
         use 5-15 normally, higher only if the game looks lightweight>, \"rationale\": \"<one short sentence>\"}}",
        ctx.game_name, ctx.app_id, ctx.process_count, ctx.total_memory_mb, ctx.available_memory_mb
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(avail: u64) -> TelemetryContext {
        TelemetryContext {
            game_name: Some("game.exe".into()),
            app_id: Some("730".into()),
            process_count: 300,
            total_memory_mb: 16000,
            available_memory_mb: avail,
        }
    }

    #[test]
    fn clamps_out_of_range_weights() {
        let p = Policy {
            cpu_weight: 0,
            rationale: String::new(),
            source: PolicySource::LocalModel,
        };
        assert_eq!(p.clamped().cpu_weight, MIN_WEIGHT);

        let p = Policy {
            cpu_weight: 5000,
            rationale: String::new(),
            source: PolicySource::LocalModel,
        };
        assert_eq!(p.clamped().cpu_weight, MAX_WEIGHT);
    }

    #[test]
    fn rule_based_throttles_harder_under_memory_pressure() {
        assert_eq!(Policy::rule_based_default(&ctx(8000)).cpu_weight, 10);
        assert_eq!(Policy::rule_based_default(&ctx(1000)).cpu_weight, MIN_WEIGHT);
    }
}
