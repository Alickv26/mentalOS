//! Backward-compatible re-exports from the providers module.
//!
//! This module re-exports types from `crate::providers::openclaw` for any
//! code that still imports from `crate::openclaw` directly.

pub use crate::providers::openclaw::{OpenClawProvider, Transport};

/// Backward-compatible alias: `OpenClawClient` is now `OpenClawProvider`.
pub type OpenClawClient = OpenClawProvider;
