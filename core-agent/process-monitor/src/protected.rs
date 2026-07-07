use crate::types::ProcessInfo;

/// Names (as reported by `/proc/<pid>/comm`, i.e. `sysinfo`'s `Process::name`) that the
/// agent must never kill, throttle, renice, or move into a throttled cgroup — regardless
/// of what any higher-level policy decides. This is a hard floor, not a suggestion:
/// callers in `cgroup-ctl` re-check `is_protected` immediately before any mutating action,
/// not just once during planning, so a stale snapshot can't smuggle a protected PID through.
const PROTECTED_NAMES: &[&str] = &[
    // init / supervision
    "systemd",
    "init",
    "kthreadd",
    // networking — killing these can strand the very SSH session managing this box
    "NetworkManager",
    "dhcpcd",
    "dhclient",
    "wpa_supplicant",
    "systemd-networkd",
    "systemd-resolved",
    "sshd",
    // display — this is the only thing standing between the user and a black screen
    "Xorg",
    "Xwayland",
    "cage",
    "kwin_wayland",
    "gnome-shell",
    "weston",
    "sway",
    "labwc",
];

/// PID 1 is always protected even if `/proc/1/comm` reports something unexpected
/// (container init, non-systemd init, etc).
const PROTECTED_PID: u32 = 1;

pub struct ProtectedSet {
    /// Extra names to protect at runtime — e.g. the active compositor binary name,
    /// resolved once at startup rather than hardcoded, since Phase 2 swaps this out
    /// for a custom Smithay compositor.
    extra_names: Vec<String>,
    /// PIDs that must never be touched this run: our own PID, our parent (likely the
    /// service supervisor), and the SSH/terminal session that launched us, if resolvable.
    extra_pids: Vec<u32>,
}

impl ProtectedSet {
    pub fn new(extra_names: Vec<String>, extra_pids: Vec<u32>) -> Self {
        Self {
            extra_names,
            extra_pids,
        }
    }

    pub fn is_protected(&self, proc: &ProcessInfo) -> bool {
        if proc.pid == PROTECTED_PID {
            return true;
        }
        if self.extra_pids.contains(&proc.pid) {
            return true;
        }
        let name_lower = proc.name.to_ascii_lowercase();
        if PROTECTED_NAMES
            .iter()
            .any(|p| p.to_ascii_lowercase() == name_lower)
        {
            return true;
        }
        self.extra_names
            .iter()
            .any(|p| p.to_ascii_lowercase() == name_lower)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(pid: u32, name: &str) -> ProcessInfo {
        ProcessInfo {
            pid,
            ppid: None,
            name: name.to_string(),
            cmd: vec![],
            exe: None,
        }
    }

    #[test]
    fn pid_1_always_protected() {
        let set = ProtectedSet::new(vec![], vec![]);
        assert!(set.is_protected(&proc(1, "anything")));
    }

    #[test]
    fn known_critical_names_protected_case_insensitively() {
        let set = ProtectedSet::new(vec![], vec![]);
        assert!(set.is_protected(&proc(42, "sshd")));
        assert!(set.is_protected(&proc(43, "NETWORKMANAGER")));
    }

    #[test]
    fn ordinary_process_not_protected() {
        let set = ProtectedSet::new(vec![], vec![]);
        assert!(!set.is_protected(&proc(1234, "some_game.exe")));
    }

    #[test]
    fn runtime_extra_names_and_pids_are_protected() {
        let set = ProtectedSet::new(vec!["my-compositor".into()], vec![999]);
        assert!(set.is_protected(&proc(999, "anything")));
        assert!(set.is_protected(&proc(1000, "my-compositor")));
    }
}
