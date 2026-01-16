//! Policy data structures and inforce loading

mod data;
pub mod loader;
pub mod generator;
pub mod adjuster;

pub use data::{Policy, QualStatus, Gender, CreditingStrategy, RollupType, BenefitBaseBucket};
pub use loader::{load_policies, load_policies_from_reader, load_default_inforce};
pub use generator::{InforceParams, InforceTemplate};
pub use adjuster::{AdjustmentParams, load_adjusted_inforce};
