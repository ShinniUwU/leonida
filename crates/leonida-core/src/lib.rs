//! Shared plumbing for every Leonida Legacy crate and tool.
//!
//! * [`config`]: where the user's files live (never inside the repository).
//! * [`policy`]: the runtime half of rule #1, refusing to write
//!   game-derived data into the source tree.
//! * [`logging`]: one tracing setup for all binaries.
//! * [`switches`]: the switches a player may touch, and which change live.
//! * [`settings`]: the settings file the game and the launcher share.

pub mod config;
pub mod graphics;
pub mod logging;
pub mod policy;
pub mod settings;
pub mod switches;

pub use config::Config;
pub use policy::{PolicyError, ensure_outside_repo};
