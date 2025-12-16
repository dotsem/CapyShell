//! Hyper-performant event bus for cross-thread communication.
//!
//! Design principles:
//! - Lock-free MPSC channels (crossbeam)
//! - Single polling timer per panel (minimal overhead)
//! - Batch processing (drain all events per tick)
//! - Zero-copy where possible
//! - Type-safe event enums (zero-cost abstraction)
//!
//! This module provides shared utilities. Each panel defines its own
//! event types in its events.rs module.

/// Create a bounded channel optimized for performance.
/// Bounded channels are faster than unbounded (no heap allocation per send).
/// Buffer size of 64 is enough for burst handling without memory bloat.
pub const CHANNEL_CAPACITY: usize = 64;
