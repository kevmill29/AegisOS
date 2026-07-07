use std::fs;
use std::path::{Path, PathBuf};

use process_monitor::{ProcessInfo, ProtectedSet};

use crate::error::CgroupError;

/// A single cgroup v2 directory. All operations are plain file writes under
/// `/sys/fs/cgroup/...` — cgroup v2 has no syscall API, the filesystem *is* the API.
///
/// This assumes the directory (and its parent's `cgroup.subtree_control`) is already
/// writable by this process. In practice that means either: (a) this process runs as
/// root, or (b) systemd created the parent slice with `Delegate=yes` and we're running
/// as the delegated unit. See ARCHITECTURE.md for why we don't try to self-elevate.
#[derive(Debug, Clone)]
pub struct CgroupHandle {
    path: PathBuf,
}

impl CgroupHandle {
    /// Creates the cgroup directory if it doesn't exist. Creating a directory under
    /// cgroup v2 is how you create a cgroup — there's no separate "create" verb.
    pub fn ensure(path: impl Into<PathBuf>) -> Result<Self, CgroupError> {
        let path = path.into();
        fs::create_dir_all(&path).map_err(|source| CgroupError::Io {
            path: path.clone(),
            source,
        })?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Enables controllers (e.g. `["cpu", "memory", "pids"]`) for *child* cgroups of
    /// this one. A controller must be enabled on the parent before children can set
    /// controller-specific files — this is cgroup v2's "no internal processes" rule
    /// surfacing as a two-step dance.
    pub fn enable_subtree_controllers(&self, controllers: &[&str]) -> Result<(), CgroupError> {
        let spec = controllers
            .iter()
            .map(|c| format!("+{c}"))
            .collect::<Vec<_>>()
            .join(" ");
        self.write_file("cgroup.subtree_control", &spec)
    }

    /// Relative CPU share, 1..=10000 (default 100). This is a *weight*, not a hard cap —
    /// it only matters when CPUs are contended, which is exactly the case we care about
    /// (game running + agent running at once).
    pub fn set_cpu_weight(&self, weight: u32) -> Result<(), CgroupError> {
        let clamped = weight.clamp(1, 10_000);
        self.write_file("cpu.weight", &clamped.to_string())
    }

    /// Hard CPU cap: `quota_us` microseconds of CPU time per `period_us` microseconds
    /// of wall time. `None` quota writes "max" (uncapped). Use sparingly — a weight is
    /// usually the right tool; a hard cap can make the agent miss its own poll interval.
    pub fn set_cpu_max(&self, quota_us: Option<u64>, period_us: u64) -> Result<(), CgroupError> {
        let quota_str = quota_us.map(|q| q.to_string()).unwrap_or_else(|| "max".into());
        self.write_file("cpu.max", &format!("{quota_str} {period_us}"))
    }

    /// Soft memory ceiling: the kernel will reclaim/throttle this cgroup under memory
    /// pressure above this many bytes, but won't OOM-kill it the way `memory.max` would.
    pub fn set_memory_high(&self, bytes: u64) -> Result<(), CgroupError> {
        self.write_file("memory.high", &bytes.to_string())
    }

    /// Moves a process into this cgroup. Requires the *caller* to supply both the
    /// `ProcessInfo` and the `ProtectedSet` so the safety check happens at the last
    /// possible moment, right next to the write — not earlier in some planning pass
    /// whose result could go stale by the time this runs.
    pub fn add_process(
        &self,
        info: &ProcessInfo,
        guard: &ProtectedSet,
    ) -> Result<(), CgroupError> {
        if guard.is_protected(info) {
            return Err(CgroupError::ProtectedProcess {
                pid: info.pid,
                name: info.name.clone(),
            });
        }
        self.write_file("cgroup.procs", &info.pid.to_string())
    }

    /// Moves the *current* process into this cgroup. Used for the agent's own
    /// self-throttling — no protected-process check needed since we can't accidentally
    /// self-select a different, protected PID.
    pub fn add_current_process(&self) -> Result<(), CgroupError> {
        self.write_file("cgroup.procs", &std::process::id().to_string())
    }

    fn write_file(&self, filename: &str, contents: &str) -> Result<(), CgroupError> {
        let file_path = self.path.join(filename);
        fs::write(&file_path, contents).map_err(|source| CgroupError::Io {
            path: file_path,
            source,
        })
    }
}

/// Confirms cgroup v2 (the "unified hierarchy") is mounted, as opposed to the legacy
/// cgroup v1 split-hierarchy layout this codebase doesn't support.
pub fn require_cgroup_v2_mounted(root: &Path) -> Result<(), CgroupError> {
    if root.join("cgroup.controllers").is_file() {
        Ok(())
    } else {
        Err(CgroupError::NotMounted(root.to_path_buf()))
    }
}

/// Extracts the cgroup path from the unified-hierarchy line of `/proc/<pid>/cgroup`
/// content. On a cgroup v2 system that line is always `0::<path>` — the `0` is the
/// (unused, single) hierarchy ID; v1's per-controller lines (`4:cpu:/...` etc) don't
/// apply here since this codebase requires v2 only.
fn parse_own_cgroup(proc_cgroup_contents: &str) -> Option<&str> {
    proc_cgroup_contents
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
}

/// Resolves the cgroup this process currently belongs to, by reading
/// `/proc/self/cgroup`. Under systemd `Delegate=yes`, this is the exact subtree
/// systemd already handed us write access to — the daemon doesn't get to pick its
/// own top-level path (that's the service unit's job via `Slice=`), it discovers
/// whatever it was actually launched into.
pub fn own_cgroup_dir(cgroup_root: &Path) -> Result<PathBuf, CgroupError> {
    let proc_path = PathBuf::from("/proc/self/cgroup");
    let contents = fs::read_to_string(&proc_path).map_err(|source| CgroupError::Io {
        path: proc_path.clone(),
        source,
    })?;
    let relative = parse_own_cgroup(&contents).ok_or_else(|| CgroupError::UnexpectedFormat {
        path: proc_path.clone(),
        detail: "no unified-hierarchy (`0::`) line found".to_string(),
    })?;
    // The line is an absolute cgroupfs path (e.g. "/user.slice/.../session.scope");
    // strip the leading '/' before joining onto cgroup_root.
    Ok(cgroup_root.join(relative.trim_start_matches('/')))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_unified_hierarchy_line() {
        let contents = "0::/system.slice/aegis-agent.service\n";
        assert_eq!(
            parse_own_cgroup(contents),
            Some("/system.slice/aegis-agent.service")
        );
    }

    #[test]
    fn returns_none_without_unified_line() {
        // What a cgroup v1 system would look like — no bare "0::" line.
        let contents = "4:cpu,cpuacct:/\n3:memory:/\n";
        assert_eq!(parse_own_cgroup(contents), None);
    }

    #[test]
    fn own_cgroup_dir_joins_onto_root() {
        let tmp = tempfile::tempdir().unwrap();
        // Can't fake /proc/self/cgroup itself, so this test exercises the joining
        // logic directly via parse_own_cgroup + manual join, matching what
        // own_cgroup_dir does internally.
        let relative = parse_own_cgroup("0::/aegis.slice/aegis-agent.service\n").unwrap();
        let joined = tmp.path().join(relative.trim_start_matches('/'));
        assert_eq!(
            joined,
            tmp.path().join("aegis.slice/aegis-agent.service")
        );
    }

    // The remaining tests point CgroupHandle at a plain tempdir rather than real
    // /sys/fs/cgroup. That can't verify the kernel accepts these writes (no real
    // controller semantics in a tempdir), but it does pin down the exact file names
    // and value formats we write — which is the part a typo or off-by-one would
    // actually break, and the part that's otherwise only checked by hand on real
    // hardware.

    #[test]
    fn ensure_creates_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("agent-normal");
        let handle = CgroupHandle::ensure(&path).unwrap();
        assert!(path.is_dir());
        assert_eq!(handle.path(), path);
    }

    #[test]
    fn set_cpu_weight_clamps_and_writes() {
        let tmp = tempfile::tempdir().unwrap();
        let handle = CgroupHandle::ensure(tmp.path()).unwrap();
        handle.set_cpu_weight(50_000).unwrap(); // above max, should clamp to 10_000
        let written = fs::read_to_string(tmp.path().join("cpu.weight")).unwrap();
        assert_eq!(written, "10000");
    }

    #[test]
    fn set_cpu_max_formats_quota_and_period() {
        let tmp = tempfile::tempdir().unwrap();
        let handle = CgroupHandle::ensure(tmp.path()).unwrap();

        handle.set_cpu_max(Some(50_000), 100_000).unwrap();
        assert_eq!(
            fs::read_to_string(tmp.path().join("cpu.max")).unwrap(),
            "50000 100000"
        );

        handle.set_cpu_max(None, 100_000).unwrap();
        assert_eq!(
            fs::read_to_string(tmp.path().join("cpu.max")).unwrap(),
            "max 100000"
        );
    }

    #[test]
    fn enable_subtree_controllers_formats_with_plus_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        let handle = CgroupHandle::ensure(tmp.path()).unwrap();
        handle.enable_subtree_controllers(&["cpu", "memory"]).unwrap();
        assert_eq!(
            fs::read_to_string(tmp.path().join("cgroup.subtree_control")).unwrap(),
            "+cpu +memory"
        );
    }

    #[test]
    fn add_current_process_writes_own_pid() {
        let tmp = tempfile::tempdir().unwrap();
        let handle = CgroupHandle::ensure(tmp.path()).unwrap();
        handle.add_current_process().unwrap();
        let written = fs::read_to_string(tmp.path().join("cgroup.procs")).unwrap();
        assert_eq!(written, std::process::id().to_string());
    }

    #[test]
    fn add_process_refuses_protected_process_without_writing() {
        let tmp = tempfile::tempdir().unwrap();
        let handle = CgroupHandle::ensure(tmp.path()).unwrap();
        let guard = ProtectedSet::new(vec![], vec![]);
        let sshd = ProcessInfo {
            pid: 42,
            ppid: None,
            name: "sshd".to_string(),
            cmd: vec![],
            exe: None,
        };

        let result = handle.add_process(&sshd, &guard);
        assert!(matches!(result, Err(CgroupError::ProtectedProcess { pid: 42, .. })));
        assert!(!tmp.path().join("cgroup.procs").exists());
    }

    #[test]
    fn add_process_writes_pid_for_unprotected_process() {
        let tmp = tempfile::tempdir().unwrap();
        let handle = CgroupHandle::ensure(tmp.path()).unwrap();
        let guard = ProtectedSet::new(vec![], vec![]);
        let game = ProcessInfo {
            pid: 4242,
            ppid: None,
            name: "csgo.exe".to_string(),
            cmd: vec![],
            exe: None,
        };

        handle.add_process(&game, &guard).unwrap();
        let written = fs::read_to_string(tmp.path().join("cgroup.procs")).unwrap();
        assert_eq!(written, "4242");
    }

    #[test]
    fn require_cgroup_v2_mounted_detects_missing_controllers_file() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(matches!(
            require_cgroup_v2_mounted(tmp.path()),
            Err(CgroupError::NotMounted(_))
        ));

        fs::write(tmp.path().join("cgroup.controllers"), "cpu memory pids").unwrap();
        assert!(require_cgroup_v2_mounted(tmp.path()).is_ok());
    }
}
