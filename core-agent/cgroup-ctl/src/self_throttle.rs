use std::path::Path;

use tracing::info;

use crate::error::CgroupError;
use crate::handle::{own_cgroup_dir, require_cgroup_v2_mounted, CgroupHandle};

const NORMAL_GROUP: &str = "normal";
const BACKGROUND_GROUP: &str = "background";

/// Lowest allowed cpu.weight. Not 1, deliberately: at weight 1 vs. a game's default 100,
/// a scheduler tick can starve the agent long enough to miss the exit event it exists to
/// detect. 10 keeps it functionally invisible under contention while still schedulable.
const BACKGROUND_CPU_WEIGHT: u32 = 10;
const NORMAL_CPU_WEIGHT: u32 = 100;

/// Implements the "Dynamic Resource Allocation" requirement: when a game is running,
/// the agent itself moves into a low-priority cgroup rather than relying on `nice`
/// alone, so it's deprioritized under both CPU *and* (via cgroup memory/IO controllers,
/// once needed) other resource contention — not just CPU scheduling.
///
/// Cgroup discovery, not creation: this does *not* assume a fixed top-level path like
/// `/sys/fs/cgroup/aegis.slice` exists or is writable. Under the intended production
/// setup (a systemd unit with `Slice=aegis.slice` and `Delegate=yes`), systemd creates
/// and delegates whatever cgroup path it sees fit — the daemon discovers that path via
/// `/proc/self/cgroup` (see [`own_cgroup_dir`]) and subdivides *that*, rather than
/// picking its own path and hoping it's already delegated.
pub struct SelfThrottle {
    normal: CgroupHandle,
    background: CgroupHandle,
}

impl SelfThrottle {
    /// Sets up (idempotently) the two sibling cgroups the agent alternates between,
    /// both children of whatever cgroup this process currently lives in.
    pub fn setup(cgroup_root: &Path) -> Result<Self, CgroupError> {
        require_cgroup_v2_mounted(cgroup_root)?;

        let own_path = own_cgroup_dir(cgroup_root)?;
        let own = CgroupHandle::ensure(&own_path)?;

        // Creating the leaf directories is always allowed regardless of controller
        // state — a cgroup can always have children, it's *processes* that are
        // restricted (see below).
        let normal = CgroupHandle::ensure(own_path.join(NORMAL_GROUP))?;
        let background = CgroupHandle::ensure(own_path.join(BACKGROUND_GROUP))?;

        // Order matters here. Cgroup v2's "no internal process" constraint means a
        // cgroup cannot simultaneously (a) hold member processes directly and (b)
        // have a controller enabled in its own `cgroup.subtree_control` (i.e. be
        // distributing resources to children). Right now our own PID is a member of
        // `own` (that's where systemd/the parent shell put it) and `own`'s
        // subtree_control is still empty, so we're compliant — but only until we
        // enable subtree_control. So: vacate `own` by moving into the `normal` leaf
        // *first*, and only then enable subtree_control on `own`. Reversing this
        // order would have the kernel reject the subtree_control write with EBUSY
        // while our PID still sits directly in `own`.
        normal.add_current_process()?;
        own.enable_subtree_controllers(&["cpu", "memory"])?;

        normal.set_cpu_weight(NORMAL_CPU_WEIGHT)?;
        background.set_cpu_weight(BACKGROUND_CPU_WEIGHT)?;

        Ok(Self { normal, background })
    }

    /// Called on game-launch detection: deprioritize the agent itself.
    pub fn engage(&self) -> Result<(), CgroupError> {
        info!(weight = BACKGROUND_CPU_WEIGHT, "self-throttling: game session detected");
        self.background.add_current_process()
    }

    /// Called on game-exit detection: restore normal priority.
    pub fn disengage(&self) -> Result<(), CgroupError> {
        info!(weight = NORMAL_CPU_WEIGHT, "restoring normal priority: game session ended");
        self.normal.add_current_process()
    }
}
