mod library;
mod vdf;

pub use library::{InstalledGame, ProtonVersion, SteamInstall};
pub use vdf::{first_value, parse, Entry};
