use crate::types::{GameSession, ProcessInfo};

/// Steam's own client process — used to confirm Steam is running at all, distinct
/// from a game session being active.
pub fn is_steam_client(proc: &ProcessInfo) -> bool {
    let name = proc.name.to_ascii_lowercase();
    name == "steam" || name == "steamwebhelper"
}

/// Proton wraps every game launch in a small supervisor called `reaper`, invoked as:
///   reaper SteamLaunch AppId=<id> -- /path/to/proton waitforexitandrun /path/to/game.exe
/// `reaper`'s job is to reap the game's process tree on exit so nothing is orphaned.
/// Its presence (and only its presence — not "wine" or "proton" substrings, which also
/// show up in unrelated library paths) is the reliable signal that a game is actually
/// running, not just installed.
fn is_reaper(proc: &ProcessInfo) -> bool {
    proc.name == "reaper"
}

/// Pulls `AppId=<id>` out of the reaper argv, if present.
fn parse_app_id(cmd: &[String]) -> Option<String> {
    cmd.iter()
        .find_map(|arg| arg.strip_prefix("AppId=").map(|id| id.to_string()))
}

/// Best-effort resolution of the actual game binary: the deepest descendant of the
/// reaper process that isn't itself Proton/wine plumbing. This is a heuristic, not a
/// guarantee — wine's process tree shape varies by title (DXVK, wine64-preloader,
/// the real .exe under it, sometimes a launcher .exe on top of that). Good enough to
/// report a name to the UI; not something throttling correctness should depend on —
/// throttling targets the whole reaper process *group*, not this one PID.
fn resolve_game_child<'a>(reaper_pid: u32, all: &'a [ProcessInfo]) -> Option<&'a ProcessInfo> {
    const WINE_INFRA: &[&str] = &["wineserver", "wine64-preloader", "wine-preloader", "proton"];

    let mut current_pid = reaper_pid;
    let mut deepest_real: Option<&ProcessInfo> = None;
    while let Some(child) = all.iter().find(|p| p.ppid == Some(current_pid)) {
        if !WINE_INFRA.contains(&child.name.to_ascii_lowercase().as_str()) {
            deepest_real = Some(child);
        }
        current_pid = child.pid;
    }
    deepest_real
}

pub fn detect_game_session(all: &[ProcessInfo]) -> Option<GameSession> {
    if let Some(reaper) = all.iter().find(|p| is_reaper(p)) {
        let app_id = parse_app_id(&reaper.cmd);
        let child = resolve_game_child(reaper.pid, all);

        return Some(GameSession {
            app_id,
            reaper_pid: reaper.pid,
            game_pid: child.map(|c| c.pid),
            game_name: child.map(|c| c.name.clone()),
        });
    }

    // Fallback: Native Linux game detection.
    // Native games don't have a 'reaper' process. They are spawned as descendants
    // of steam or steamwebhelper and run from the steamapps directory.
    let steam_pids: Vec<u32> = all.iter()
        .filter(|p| is_steam_client(p))
        .map(|p| p.pid)
        .collect();

    if !steam_pids.is_empty() {
        for proc in all {
            let cmd_str = proc.cmd.join(" ");
            let exe_str = proc.exe.as_deref().unwrap_or("");
            
            if cmd_str.contains("/steamapps/common/") || exe_str.contains("/steamapps/common/") {
                // Verify it's actually a descendant of Steam to avoid false positives
                let mut is_steam_child = false;
                let mut current_ppid = proc.ppid;
                
                while let Some(ppid) = current_ppid {
                    if steam_pids.contains(&ppid) {
                        is_steam_child = true;
                        break;
                    }
                    // Traverse up the tree
                    current_ppid = all.iter().find(|p| p.pid == ppid).and_then(|p| p.ppid);
                }

                if is_steam_child {
                    return Some(GameSession {
                        app_id: None, // Native game ID isn't exposed in argv like it is for Proton
                        reaper_pid: proc.pid, // Use the game's own PID as the session root for throttling
                        game_pid: Some(proc.pid),
                        game_name: Some(proc.name.clone()),
                    });
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(pid: u32, ppid: Option<u32>, name: &str, cmd: &[&str]) -> ProcessInfo {
        ProcessInfo {
            pid,
            ppid,
            name: name.to_string(),
            cmd: cmd.iter().map(|s| s.to_string()).collect(),
            exe: None,
        }
    }

    #[test]
    fn no_reaper_means_no_session() {
        let procs = vec![proc(1, None, "systemd", &[]), proc(2, Some(1), "steam", &[])];
        assert!(detect_game_session(&procs).is_none());
    }

    #[test]
    fn detects_session_and_parses_app_id() {
        let procs = vec![
            proc(1, None, "systemd", &[]),
            proc(
                10,
                Some(1),
                "reaper",
                &["reaper", "SteamLaunch", "AppId=730", "--", "proton", "waitforexitandrun"],
            ),
            proc(11, Some(10), "wine64-preloader", &[]),
            proc(12, Some(11), "csgo.exe", &[]),
        ];
        let session = detect_game_session(&procs).expect("session detected");
        assert_eq!(session.app_id.as_deref(), Some("730"));
        assert_eq!(session.reaper_pid, 10);
        // resolves through the wine preloader hop to the actual game binary
        assert_eq!(session.game_pid, Some(12));
        assert_eq!(session.game_name.as_deref(), Some("csgo.exe"));
    }

    #[test]
    fn detects_native_linux_game() {
        let procs = vec![
            proc(1, None, "systemd", &[]),
            proc(2, Some(1), "steam", &[]),
            // A native game spawned by steam
            proc(
                3,
                Some(2),
                "cs2",
                &["/home/gamer/.local/share/Steam/steamapps/common/Counter-Strike Global Offensive/game/bin/linuxsteamrt64/cs2"],
            ),
        ];
        let session = detect_game_session(&procs).expect("native session detected");
        assert_eq!(session.app_id, None); // We don't parse AppId for native games yet
        assert_eq!(session.reaper_pid, 3); // The game itself is the session root
        assert_eq!(session.game_pid, Some(3));
        assert_eq!(session.game_name.as_deref(), Some("cs2"));
    }
}
