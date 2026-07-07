use sysinfo::System;

use crate::protected::ProtectedSet;
use crate::steam::{detect_game_session, is_steam_client};
use crate::types::{GameSession, ProcessInfo};

/// Wraps `sysinfo::System` with the specific refresh policy the daemon needs (processes
/// only — CPU/memory-per-process refresh is comparatively expensive and this daemon
/// polls frequently, so we don't refresh anything the detection logic doesn't use).
pub struct ProcessMonitor {
    system: System,
    protected: ProtectedSet,
}

impl ProcessMonitor {
    pub fn new(protected: ProtectedSet) -> Self {
        Self {
            system: System::new(),
            protected,
        }
    }

    /// Re-scans the process table. Call this once per poll tick before querying state.
    /// Explicit refresh kind, not the default: detection parses `AppId=` out of argv,
    /// and sysinfo only populates `cmd`/`exe` if asked (verified empirically — with
    /// the default refresh, a live reaper process was detected but its AppId came
    /// back None because `cmd()` was empty).
    pub fn refresh(&mut self) {
        self.system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            sysinfo::ProcessRefreshKind::new()
                .with_cmd(sysinfo::UpdateKind::Always)
                .with_exe(sysinfo::UpdateKind::OnlyIfNotSet),
        );
    }

    pub fn snapshot(&self) -> Vec<ProcessInfo> {
        self.system
            .processes()
            .values()
            .map(|p| ProcessInfo {
                pid: p.pid().as_u32(),
                ppid: p.parent().map(|p| p.as_u32()),
                name: p.name().to_string_lossy().into_owned(),
                cmd: p
                    .cmd()
                    .iter()
                    .map(|s| s.to_string_lossy().into_owned())
                    .collect(),
                exe: p.exe().map(|p| p.to_string_lossy().into_owned()),
            })
            .collect()
    }

    pub fn steam_running(&self) -> bool {
        self.system
            .processes()
            .values()
            .any(|p| is_steam_client(&to_info(p)))
    }

    pub fn active_game_session(&self) -> Option<GameSession> {
        detect_game_session(&self.snapshot())
    }

    /// Whether a given PID is off-limits for any throttle/kill action, per the safety
    /// guardrails in [`crate::protected`]. Always re-check this immediately before a
    /// mutating action — don't cache the result from an earlier snapshot.
    pub fn is_protected_pid(&self, pid: u32) -> bool {
        self.snapshot()
            .iter()
            .find(|p| p.pid == pid)
            .map(|p| self.protected.is_protected(p))
            // If the PID no longer exists, treat it as protected-by-default (fail closed):
            // there's nothing useful to throttle and no reason to guess.
            .unwrap_or(true)
    }
}

fn to_info(p: &sysinfo::Process) -> ProcessInfo {
    ProcessInfo {
        pid: p.pid().as_u32(),
        ppid: p.parent().map(|p| p.as_u32()),
        name: p.name().to_string_lossy().into_owned(),
        cmd: vec![],
        exe: None,
    }
}
