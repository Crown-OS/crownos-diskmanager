//! Storage discovery and management for CrownOS, built on UDisks2.
//!
//! The crate never needs root: every privileged action is a UDisks2 method
//! authorised by polkit, and raw device access for disk images goes through a
//! file descriptor UDisks2 hands over.

mod client;
pub mod command;
pub mod error;
#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures;
pub mod guard;
pub mod journal;
pub mod limits;
pub mod model;
mod object_import;
mod object_map;
mod operations;
pub mod recording;
mod service;
mod snapshot;
mod util;
mod watcher;

pub use command::{Command, Event};
pub use error::{StorageError, UserError};
pub use service::run;
