use serde::Serialize;

/// A snapshot of one OS process, trimmed to the fields the daemon actually reasons about.
#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: Option<u32>,
    pub name: String,
    pub cmd: Vec<String>,
    pub exe: Option<String>,
}

impl ProcessInfo {
    pub fn cmdline_joined(&self) -> String {
        self.cmd.join(" ")
    }
}

/// A detected Steam/Proton game session, derived from the process tree rather than
/// any Steam API — the agent has no privileged hook into Steam, it infers state from
/// what the OS can already see.
#[derive(Debug, Clone, Serialize)]
pub struct GameSession {
    /// Steam AppID, parsed out of the `reaper ... AppId=<id>` argv Steam launches games with.
    pub app_id: Option<String>,
    /// PID of the Proton `reaper` process wrapping this game launch.
    pub reaper_pid: u32,
    /// Best-guess PID of the actual game binary (child of reaper / wine preloader), if resolved.
    pub game_pid: Option<u32>,
    pub game_name: Option<String>,
}
