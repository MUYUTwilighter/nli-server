//! NetherLink API v2 clean-slate implementation.
//!
//! Phase 0 intentionally exposes no business routes.

pub mod platform;
pub mod v2;

/// Cargo package version embedded in the binary.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
