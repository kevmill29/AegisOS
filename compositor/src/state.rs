use crate::Backend;
use tracing::{info, warn};

/// Holds the compositor's global state. In the completed implementation this
/// owns the Smithay `Display`, the `Space` of mapped windows, the seat/input
/// state, and the renderer. The scaffold keeps the shape and the lifecycle so
/// the backend wiring and the state machine can be filled in independently.
pub struct CompositorState {
    backend: Backend,
}

impl CompositorState {
    pub fn new(backend: Backend) -> Self {
        Self { backend }
    }

    /// The compositor lifecycle, in the order the real implementation runs it.
    /// Each `todo_*` call marks a Smithay integration boundary that is the next
    /// implementation target; they're separated so they can be built and tested
    /// one at a time against the winit backend before the udev backend is wired.
    pub fn run(&mut self) {
        self.init_wayland_display();
        self.init_backend();
        self.init_shell();
        self.event_loop();
    }

    /// Create the Wayland `Display`, expose the compositor/xdg-shell/seat/output
    /// globals, and start listening on a socket in XDG_RUNTIME_DIR. Clients
    /// (the Electron frontend, later Proton games) connect here.
    fn init_wayland_display(&self) {
        info!("todo: create wl_display, advertise globals (compositor, xdg_shell, seat, output)");
    }

    /// Bring up the selected backend's renderer + input. Winit gives a nested
    /// window and forwards host input; udev claims DRM/KMS scanout and libinput
    /// devices directly.
    fn init_backend(&self) {
        match self.backend {
            Backend::Winit => info!("todo: winit backend — nested window, GL renderer, host input"),
            Backend::Udev => info!("todo: udev backend — DRM/KMS scanout, libinput seat"),
        }
    }

    /// Aegis-specific shell policy: exactly one primary surface fullscreen (the
    /// sphere UI), and on game launch, hand scanout to the game's surface while
    /// the agent goes dormant — the compositor-side half of the "Sleep Mode"
    /// contract the agent-daemon already emits events for.
    fn init_shell(&self) {
        info!("todo: kiosk shell policy — single fullscreen surface, game-launch handoff");
    }

    fn event_loop(&mut self) {
        // The real loop dispatches the Wayland display, pumps backend events
        // (input, redraw, output hotplug), and renders damaged regions. The
        // scaffold exits immediately so the binary is runnable and CI-safe
        // rather than blocking forever on a display that isn't wired yet.
        warn!("event loop not yet implemented — scaffold exits cleanly");
    }
}
