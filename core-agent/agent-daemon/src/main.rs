mod events;

use std::path::Path;
use std::time::Duration;

use cgroup_ctl::SelfThrottle;
use process_monitor::{GameSession, ProcessMonitor, ProtectedSet};
use tracing::{info, warn};

use events::{Event, EventBus};

const CGROUP_ROOT: &str = "/sys/fs/cgroup";
const POLL_INTERVAL: Duration = Duration::from_secs(2);
/// Localhost-only; the frontend connects here. Not configurable yet — one daemon
/// per machine is the design, so a fixed port doubles as a "second instance"
/// tripwire (bind fails loudly).
const EVENT_BUS_ADDR: &str = "127.0.0.1:48620";

/// How long each phase of a simulated game session lasts.
const SIMULATE_PHASE: Duration = Duration::from_secs(10);

enum SessionState {
    Idle,
    Active(GameSession),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let simulate = std::env::args().any(|a| a == "--simulate");

    info!(simulate, "Aegis core-agent starting");

    let bus = EventBus::start(EVENT_BUS_ADDR).await?;

    if simulate {
        // Simulation replaces the real monitor entirely — no cgroup writes, no
        // process scanning, just the event contract the frontend codes against.
        run_simulated(&bus).await;
        return Ok(());
    }

    // Enumerate the Steam library once at startup. Not required for detection
    // (that's inferred live from the reaper process), but it lets the agent map
    // an AppId to a game name and know what's installed — logged now, consumed
    // by the future LLM policy and frontend. Degrades silently if Steam isn't
    // found (the common case on this dev box's Linux side).
    match steam_integration::SteamInstall::discover(None) {
        Some(steam) => {
            let games = steam.installed_games();
            info!(count = games.len(), root = %steam.root.display(), "steam library found");
        }
        None => info!("no steam install found (detection still works via reaper)"),
    }

    let protected = ProtectedSet::new(vec![], vec![std::process::id()]);
    let mut monitor = ProcessMonitor::new(protected);

    // The throttle brain. With the `llm` feature it consults the local model
    // (then cloud, then rules); without it, it's a no-op and SelfThrottle uses
    // its fixed weights. Either way the daemon behaves — the LLM only tunes the
    // number, it's never in the critical path of detecting a game.
    #[cfg(feature = "llm")]
    let brain = llm_orchestrator::Orchestrator::new();

    // Self-throttling requires write access to the cgroup tree (root, or a systemd
    // Delegate=yes slice). During local dev without either, we degrade to
    // monitor-only mode rather than refusing to start.
    let throttle = match SelfThrottle::setup(Path::new(CGROUP_ROOT)) {
        Ok(t) => {
            info!("cgroup self-throttling enabled");
            Some(t)
        }
        Err(e) => {
            warn!(error = %e, "cgroup self-throttling unavailable, running monitor-only");
            None
        }
    };

    let mut state = SessionState::Idle;
    let mut ticker = tokio::time::interval(POLL_INTERVAL);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                monitor.refresh();
                let started = handle_tick(&monitor, &mut state, throttle.as_ref(), &bus);
                // On a fresh game launch, consult the throttle brain (llm feature
                // only) and apply its recommended weight to the live background
                // cgroup. SelfThrottle::engage already throttled with a safe
                // default the instant the game was seen; this only *refines* that
                // number afterward, so a slow or unavailable model can never delay
                // or block the deprioritization that already happened.
                #[cfg(feature = "llm")]
                if let Some(session) = started {
                    monitor.refresh_memory();
                    let ctx = llm_orchestrator::TelemetryContext {
                        game_name: session.game_name.clone(),
                        app_id: session.app_id.clone(),
                        process_count: monitor.snapshot().len(),
                        total_memory_mb: monitor.total_memory_mb(),
                        available_memory_mb: monitor.available_memory_mb(),
                    };
                    let policy = brain.decide(&ctx);
                    info!(
                        source = ?policy.source,
                        cpu_weight = policy.cpu_weight,
                        rationale = %policy.rationale,
                        "throttle brain recommendation"
                    );
                    if let Some(t) = throttle.as_ref() {
                        match t.apply_background_weight(policy.cpu_weight) {
                            Ok(()) => info!(
                                cpu_weight = policy.cpu_weight,
                                "applied throttle-brain cpu.weight to live cgroup"
                            ),
                            Err(e) => warn!(error = %e, "failed to apply brain cpu.weight"),
                        }
                    }
                }
                #[cfg(not(feature = "llm"))]
                let _ = started;
            }
            _ = tokio::signal::ctrl_c() => {
                info!("shutdown signal received");
                break;
            }
        }
    }

    Ok(())
}

/// Returns the newly-started `GameSession` when this tick transitioned Idle →
/// Active, so the caller can run start-of-session work (throttle-brain
/// consultation) without duplicating the detection logic.
fn handle_tick(
    monitor: &ProcessMonitor,
    state: &mut SessionState,
    throttle: Option<&SelfThrottle>,
    bus: &EventBus,
) -> Option<GameSession> {
    let detected = monitor.active_game_session();

    match (&state, detected) {
        (SessionState::Idle, Some(session)) => {
            info!(app_id = ?session.app_id, game = ?session.game_name, "game session detected");
            bus.emit(&Event::from_session(&session));
            if let Some(t) = throttle {
                if let Err(e) = t.engage() {
                    warn!(error = %e, "failed to self-throttle");
                }
            }
            *state = SessionState::Active(session.clone());
            return Some(session);
        }
        (SessionState::Active(prev), Some(session)) if prev.reaper_pid != session.reaper_pid => {
            // Previous reaper exited and a new one appeared inside the same poll
            // gap (e.g. quick game switch) — treat as a fresh session rather than
            // silently keeping stale state.
            info!(app_id = ?session.app_id, "game session changed");
            *state = SessionState::Active(session);
        }
        (SessionState::Active(_), None) => {
            info!("game session ended");
            bus.emit(&Event::SleepModeEnded);
            if let Some(t) = throttle {
                if let Err(e) = t.disengage() {
                    warn!(error = %e, "failed to restore normal priority");
                }
            }
            *state = SessionState::Idle;
        }
        _ => {}
    }
    None
}

/// Alternates fake game launch/exit events forever, so the frontend's full state
/// machine can be exercised on a machine with no Steam install at all.
async fn run_simulated(bus: &EventBus) {
    info!(phase_secs = SIMULATE_PHASE.as_secs(), "running in simulate mode");
    loop {
        tokio::select! {
            _ = tokio::time::sleep(SIMULATE_PHASE) => {}
            _ = tokio::signal::ctrl_c() => { info!("shutdown signal received"); return; }
        }
        bus.emit(&Event::SleepModeInitiated {
            app_id: Some("730".to_string()),
            game_name: Some("simulated-game.exe".to_string()),
        });

        tokio::select! {
            _ = tokio::time::sleep(SIMULATE_PHASE) => {}
            _ = tokio::signal::ctrl_c() => { info!("shutdown signal received"); return; }
        }
        bus.emit(&Event::SleepModeEnded);
    }
}
