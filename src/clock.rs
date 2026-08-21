//! Clock sources, swapped per target.
//!
//! On wasm32-unknown-unknown there is no platform clock, so both
//! `std::time::Instant::now()` and `std::time::SystemTime::now()` panic at
//! runtime. `web-time` provides API-compatible replacements backed by
//! `performance.now()` and `Date.now()`. Every module that needs a clock goes
//! through here rather than reaching for `std::time` directly.

/// `Duration` is pure arithmetic with no platform clock behind it, so the std
/// type is correct on every target.
pub use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
pub use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[cfg(target_arch = "wasm32")]
pub use web_time::{Instant, SystemTime, UNIX_EPOCH};
