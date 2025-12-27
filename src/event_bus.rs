//! Event bus for cross-thread communication.
//!
//! Design principles:
//! - Broadcast channel (tokio) - all subscribers receive every event
//! - Single polling timer per panel (minimal overhead)
//! - Batch processing (drain all events per tick)
//! - Type-safe event enums (zero-cost abstraction)
//!
//! This module provides shared utilities. Each panel defines its own
//! event types in its events.rs module.

/// Broadcast channel capacity.
/// 64 is enough for burst handling without memory bloat.
/// Lagging receivers will skip old events (we only care about latest).
pub const CHANNEL_CAPACITY: usize = 64;
