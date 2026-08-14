//! Isolated 26.2 simulation engines (light, redstone, fluids, mob spawn).
//!
//! These modules are **not** wired into `neutron-server` yet. They own their
//! own in-memory block maps and are exercised by unit tests / benches.
//! Do not add a worldgen dependency for "just a block id" — keep this crate
//! self-contained until a shared `BlockState` exists.
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod block;
pub mod fluid;
pub mod light;
pub mod redstone;
pub mod spawn;
