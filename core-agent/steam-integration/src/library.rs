use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tracing::debug;

use crate::vdf;

/// An installed Steam game, as recorded by an appmanifest_<appid>.acf.
#[derive(Debug, Clone, Serialize)]
pub struct InstalledGame {
    pub app_id: String,
    pub name: String,
    pub install_dir: PathBuf,
    /// Raw Steam StateFlags bitfield; 4 = fully installed. Kept raw rather than
    /// interpreted because the daemon mostly wants "does this AppId exist here"
    /// and over-modeling Valve's undocumented flags invites breakage.
    pub state_flags: u32,
}

/// A Proton build Steam has installed (official builds live in steamapps/common,
/// community builds like GE-Proton in compatibilitytools.d).
#[derive(Debug, Clone, Serialize)]
pub struct ProtonVersion {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SteamInstall {
    pub root: PathBuf,
}

impl SteamInstall {
    /// Finds a Steam installation by probing well-known roots. Order matters:
    /// native Linux paths first (the real target), then the Windows install as
    /// seen from WSL (the dev bench). An explicit override always wins.
    pub fn discover(override_root: Option<&Path>) -> Option<Self> {
        if let Some(root) = override_root {
            return Self::at(root);
        }
        let home = std::env::var("HOME").unwrap_or_default();
        let candidates = [
            format!("{home}/.steam/steam"),
            format!("{home}/.local/share/Steam"),
            format!("{home}/.var/app/com.valvesoftware.Steam/.local/share/Steam"), // flatpak
            "/mnt/c/Program Files (x86)/Steam".to_string(),
        ];
        candidates.iter().find_map(|c| Self::at(Path::new(c)))
    }

    fn at(root: &Path) -> Option<Self> {
        root.join("steamapps").is_dir().then(|| Self {
            root: root.to_path_buf(),
        })
    }

    /// All steamapps directories: the main one plus any extra library folders
    /// registered in libraryfolders.vdf (e.g. a games drive).
    pub fn library_dirs(&self) -> Vec<PathBuf> {
        let main = self.root.join("steamapps");
        let mut dirs = vec![main.clone()];

        if let Ok(text) = fs::read_to_string(main.join("libraryfolders.vdf")) {
            for entry in vdf::parse(&text) {
                if entry.key.eq_ignore_ascii_case("path") {
                    let lib = PathBuf::from(&entry.value).join("steamapps");
                    if lib.is_dir() && !dirs.contains(&lib) {
                        dirs.push(lib);
                    }
                }
            }
        }
        dirs
    }

    pub fn installed_games(&self) -> Vec<InstalledGame> {
        let mut games = Vec::new();
        for lib in self.library_dirs() {
            let Ok(dir) = fs::read_dir(&lib) else { continue };
            for entry in dir.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if !name.starts_with("appmanifest_") || !name.ends_with(".acf") {
                    continue;
                }
                match fs::read_to_string(entry.path()) {
                    Ok(text) => {
                        if let Some(game) = parse_manifest(&text, &lib) {
                            games.push(game);
                        }
                    }
                    Err(e) => debug!(file = %entry.path().display(), error = %e, "unreadable manifest"),
                }
            }
        }
        games.sort_by(|a, b| a.name.cmp(&b.name));
        games
    }

    pub fn proton_versions(&self) -> Vec<ProtonVersion> {
        let mut found = Vec::new();
        for lib in self.library_dirs() {
            let common = lib.join("common");
            let Ok(dir) = fs::read_dir(&common) else { continue };
            for entry in dir.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                // Official builds are literally named "Proton <something>";
                // proton_dir_is_real guards against a coincidentally-named game.
                if name.starts_with("Proton") && entry.path().join("proton").is_file() {
                    found.push(ProtonVersion {
                        name,
                        path: entry.path(),
                    });
                }
            }
        }
        let compat = self.root.join("compatibilitytools.d");
        if let Ok(dir) = fs::read_dir(&compat) {
            for entry in dir.flatten() {
                if entry.path().join("proton").is_file() {
                    found.push(ProtonVersion {
                        name: entry.file_name().to_string_lossy().into_owned(),
                        path: entry.path(),
                    });
                }
            }
        }
        found.sort_by(|a, b| a.name.cmp(&b.name));
        found
    }
}

fn parse_manifest(text: &str, library: &Path) -> Option<InstalledGame> {
    let entries = vdf::parse(text);
    let app_id = vdf::first_value(&entries, "appid")?.to_string();
    let name = vdf::first_value(&entries, "name")?.to_string();
    let install_dir = library
        .join("common")
        .join(vdf::first_value(&entries, "installdir")?);
    let state_flags = vdf::first_value(&entries, "StateFlags")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    Some(InstalledGame {
        app_id,
        name,
        install_dir,
        state_flags,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_manifest_extracts_fields() {
        let acf = r#""AppState" { "appid" "730" "name" "Counter-Strike 2" "installdir" "Counter-Strike Global Offensive" "StateFlags" "4" }"#;
        let game = parse_manifest(acf, Path::new("/lib/steamapps")).unwrap();
        assert_eq!(game.app_id, "730");
        assert_eq!(game.name, "Counter-Strike 2");
        assert_eq!(
            game.install_dir,
            Path::new("/lib/steamapps/common/Counter-Strike Global Offensive")
        );
        assert_eq!(game.state_flags, 4);
    }

    #[test]
    fn parse_manifest_rejects_incomplete() {
        assert!(parse_manifest(r#""AppState" { "appid" "730" }"#, Path::new("/x")).is_none());
    }
}
