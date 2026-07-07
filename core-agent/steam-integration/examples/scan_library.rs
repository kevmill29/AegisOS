//! Scans the real Steam install visible from this machine and prints what
//! Aegis would know about it. This is the live verification for Phase 4.
use steam_integration::SteamInstall;

fn main() {
    let Some(steam) = SteamInstall::discover(None) else {
        println!("no Steam install found");
        return;
    };
    println!("steam root: {}", steam.root.display());
    println!("library dirs:");
    for d in steam.library_dirs() {
        println!("  {}", d.display());
    }
    println!("installed games:");
    for g in steam.installed_games() {
        println!(
            "  [{}] {} (state={}) -> {}",
            g.app_id,
            g.name,
            g.state_flags,
            g.install_dir.display()
        );
    }
    println!("proton versions:");
    for p in steam.proton_versions() {
        println!("  {} -> {}", p.name, p.path.display());
    }
}
