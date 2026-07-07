//! Debug helper: prints what the monitor's refresh actually sees for any process
//! named "reaper". Run one alongside: `/tmp/aegis-test/reaper SteamLaunch AppId=730 ...`
use process_monitor::{ProcessMonitor, ProtectedSet};

fn main() {
    let mut monitor = ProcessMonitor::new(ProtectedSet::new(vec![], vec![]));
    monitor.refresh();
    let snapshot = monitor.snapshot();
    println!("total processes seen: {}", snapshot.len());
    for p in snapshot.iter().filter(|p| p.name.contains("reaper") || p.name.contains("sleep")) {
        println!("{:?}", p);
    }
    println!("session: {:?}", monitor.active_game_session());
}
