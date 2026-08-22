//! Feature ports ("T4"): one module per feature family.
//!
//! - `simple`      desert_well / freeze_top_layer / spike / bamboo / monster_room
//! - `lake`        LakeFeature
//! - `sequence`    SequenceFeature + inline placed-feature pipeline
//!                   (pending consolidation with dispatch pipeline)
//! - `underground` speleothem clusters / large dripstone / fossil / geode
//! - `ice`         IcebergFeature

/// Vanilla sea level (shared by iceberg/lake placement heights).
pub(crate) const SEA_LEVEL: i32 = 63;

mod ice;
mod lake;
mod sequence;
mod simple;
mod underground;

pub(crate) use ice::*;
pub(crate) use lake::*;
pub(crate) use sequence::*;
pub(crate) use simple::*;
pub(crate) use underground::*;
