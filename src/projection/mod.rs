//! Projection engine for single and multi-policy projections

mod state;
mod engine;
mod cashflows;

pub use state::ProjectionState;
pub use engine::{ProjectionEngine, ProjectionConfig, CreditingApproach};
pub use cashflows::{CashflowRow, ProjectionResult};

// ============================================================================
// Default Crediting Rates
// ============================================================================
// These are the standard crediting rates used for pricing projections.
// - Fixed policies receive monthly compounding of the fixed rate
// - Indexed policies receive annual credits at policy anniversary

/// Default annual credited rate for Fixed crediting strategy (2.75%)
pub const DEFAULT_FIXED_ANNUAL_RATE: f64 = 0.0275;

/// Default annual credited rate for Indexed crediting strategy (3.78%)
pub const DEFAULT_INDEXED_ANNUAL_RATE: f64 = 0.0378;
