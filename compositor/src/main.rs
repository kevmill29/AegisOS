//! Aegis custom compositor — Phase 3 scaffold.
//!
//! The charter's compositor path is: Linux → Wayland → cage (kiosk) → custom
//! Smithay compositor. `launch-kiosk.sh` is the working cage stage today; this
//! binary is the successor, which replaces cage with a compositor we control so
//! Aegis can own damage tracking, the audio-reactive background, input routing
//! to the agent (Ctrl+Alt+T, voice), and game-launch surface handoff directly
//! rather than through a generic kiosk.
//!
//! Scaffold status: this establishes the crate, the backend-selection strategy,
//! and the event-loop skeleton. The Smithay state machine (globals, seat, output,
//! surface commit handling) is stubbed at the boundaries marked below — those are
//! the next implementation targets, and each is isolated so the backend can be
//! swapped (winit for dev → udev/DRM for bare metal) without touching them.

use tracing::info;

mod state;

use state::CompositorState;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Aegis compositor starting (scaffold)");

    let backend = select_backend();
    info!(?backend, "selected backend");

    let mut state = CompositorState::new(backend);
    state.run();
}

/// Which Smithay backend to drive. On a dev machine inside an existing Wayland
/// or X session (WSLg included) we nest under winit; on bare metal we take over
/// the GPU directly via udev/DRM/KMS. Detection is by environment: a compositor
/// that finds WAYLAND_DISPLAY/DISPLAY set is nested, otherwise it's on the
/// hardware.
#[derive(Debug, Clone, Copy)]
pub enum Backend {
    /// Nested window inside an existing compositor — dev/testing.
    Winit,
    /// Direct DRM/KMS + libinput on real hardware — the deployment target.
    Udev,
}

fn select_backend() -> Backend {
    let nested = std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var_os("DISPLAY").is_some();
    if nested {
        Backend::Winit
    } else {
        Backend::Udev
    }
}
