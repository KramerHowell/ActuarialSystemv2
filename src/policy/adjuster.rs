//! Adjust loaded policies based on parameters
//!
//! This module provides functions to load the base pricing_inforce.csv
//! and apply parameter adjustments (fixed %, gender weights, etc.)

use super::{Policy, QualStatus, Gender, CreditingStrategy, load_default_inforce};
use serde::{Deserialize, Serialize};
use std::error::Error;

/// Parameters for adjusting the inforce
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjustmentParams {
    /// Target percentage for Fixed strategy (0.0 to 1.0)
    /// Base CSV has 25% Fixed, 75% Indexed
    /// Setting to 0.50 would make it 50/50
    #[serde(default = "default_fixed_pct")]
    pub fixed_pct: f64,

    /// Multiplier for male policies (1.0 = no change)
    #[serde(default = "default_one")]
    pub male_mult: f64,

    /// Multiplier for female policies (1.0 = no change)
    #[serde(default = "default_one")]
    pub female_mult: f64,

    /// Multiplier for qualified policies (1.0 = no change)
    #[serde(default = "default_one")]
    pub qual_mult: f64,

    /// Multiplier for non-qualified policies (1.0 = no change)
    #[serde(default = "default_one")]
    pub nonqual_mult: f64,

    /// Benefit base bonus as a fraction (0.30 = 30%, meaning BB = Premium × 1.3)
    /// Base CSV has 30% bonus baked in
    /// Setting to 0.40 would make BB = Premium × 1.4
    #[serde(default = "default_bb_bonus")]
    pub bb_bonus: f64,

    /// Target total premium (default: $100M)
    #[serde(default = "default_target_premium")]
    pub target_premium: f64,
}

fn default_fixed_pct() -> f64 { 0.25 }
fn default_one() -> f64 { 1.0 }
fn default_bb_bonus() -> f64 { 0.30 }
fn default_target_premium() -> f64 { 100_000_000.0 }

impl Default for AdjustmentParams {
    fn default() -> Self {
        Self {
            fixed_pct: 0.25,
            male_mult: 1.0,
            female_mult: 1.0,
            qual_mult: 1.0,
            nonqual_mult: 1.0,
            bb_bonus: 0.30,
            target_premium: 100_000_000.0,
        }
    }
}

/// Load policies from CSV and apply parameter adjustments
pub fn load_adjusted_inforce(params: &AdjustmentParams) -> Result<Vec<Policy>, Box<dyn Error>> {
    // Load base policies
    let base_policies = load_default_inforce()?;

    // If all params are default, return as-is
    if is_default_params(params) {
        return Ok(base_policies);
    }

    // Apply adjustments
    let adjusted = adjust_policies(base_policies, params);
    Ok(adjusted)
}

fn is_default_params(params: &AdjustmentParams) -> bool {
    (params.fixed_pct - 0.25).abs() < 1e-9 &&
    (params.male_mult - 1.0).abs() < 1e-9 &&
    (params.female_mult - 1.0).abs() < 1e-9 &&
    (params.qual_mult - 1.0).abs() < 1e-9 &&
    (params.nonqual_mult - 1.0).abs() < 1e-9 &&
    (params.bb_bonus - 0.30).abs() < 1e-9 &&
    (params.target_premium - 100_000_000.0).abs() < 1.0
}

fn adjust_policies(mut policies: Vec<Policy>, params: &AdjustmentParams) -> Vec<Policy> {
    // Base CSV has 25% Fixed, 75% Indexed
    // To adjust to new fixed_pct, we scale:
    // - Fixed policies: multiply by (new_fixed_pct / 0.25)
    // - Indexed policies: multiply by ((1 - new_fixed_pct) / 0.75)
    let fixed_scale = params.fixed_pct / 0.25;
    let indexed_scale = (1.0 - params.fixed_pct) / 0.75;

    // First pass: calculate total weighted premium for rescaling
    let mut total_weighted = 0.0;
    for policy in &policies {
        let mut weight = 1.0;

        // Strategy adjustment
        weight *= match policy.crediting_strategy {
            CreditingStrategy::Fixed => fixed_scale,
            CreditingStrategy::Indexed => indexed_scale,
        };

        // Gender adjustment
        weight *= match policy.gender {
            Gender::Male => params.male_mult,
            Gender::Female => params.female_mult,
        };

        // Qual status adjustment
        weight *= match policy.qual_status {
            QualStatus::Q => params.qual_mult,
            QualStatus::N => params.nonqual_mult,
        };

        total_weighted += policy.initial_premium * weight;
    }

    // Scale factor to hit target premium
    let premium_scale = if total_weighted > 0.0 {
        params.target_premium / total_weighted
    } else {
        1.0
    };

    // BB bonus factor: ratio of new bonus to base 30%
    // Base CSV has BB = Premium × 1.3 (30% bonus)
    // If new bb_bonus is 0.40, we want BB = Premium × 1.4
    // So scale factor is (1 + 0.40) / (1 + 0.30) = 1.4 / 1.3
    let bb_bonus_factor = (1.0 + params.bb_bonus) / 1.3;

    // Second pass: apply adjustments
    for policy in &mut policies {
        let mut weight = 1.0;

        // Strategy adjustment
        weight *= match policy.crediting_strategy {
            CreditingStrategy::Fixed => fixed_scale,
            CreditingStrategy::Indexed => indexed_scale,
        };

        // Gender adjustment
        weight *= match policy.gender {
            Gender::Male => params.male_mult,
            Gender::Female => params.female_mult,
        };

        // Qual status adjustment
        weight *= match policy.qual_status {
            QualStatus::Q => params.qual_mult,
            QualStatus::N => params.nonqual_mult,
        };

        // Apply premium scaling
        let total_scale = weight * premium_scale;
        policy.initial_premium *= total_scale;
        policy.initial_pols *= total_scale;
        policy.percentage *= weight;  // Percentage doesn't get premium_scale

        // Apply BB adjustment (premium scale + bonus adjustment)
        policy.initial_benefit_base *= total_scale * bb_bonus_factor;

        // Update bonus field to reflect new BB bonus
        if (params.bb_bonus - 0.30).abs() > 1e-9 {
            // Set the policy bonus to reflect the new BB/Premium ratio
            policy.bonus = params.bb_bonus;
        }
    }

    policies
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_params_unchanged() {
        let params = AdjustmentParams::default();
        let policies = load_adjusted_inforce(&params).expect("Failed to load");

        // Should be same as raw CSV
        assert_eq!(policies.len(), 2800);

        let total_premium: f64 = policies.iter().map(|p| p.initial_premium).sum();
        assert!((total_premium - 100_000_000.0).abs() < 1.0);
    }

    #[test]
    fn test_fifty_fifty_split() {
        let params = AdjustmentParams {
            fixed_pct: 0.50,
            ..Default::default()
        };
        let policies = load_adjusted_inforce(&params).expect("Failed to load");

        let fixed_premium: f64 = policies.iter()
            .filter(|p| p.crediting_strategy == CreditingStrategy::Fixed)
            .map(|p| p.initial_premium)
            .sum();
        let indexed_premium: f64 = policies.iter()
            .filter(|p| p.crediting_strategy == CreditingStrategy::Indexed)
            .map(|p| p.initial_premium)
            .sum();

        // Should be approximately 50/50
        let fixed_pct = fixed_premium / (fixed_premium + indexed_premium);
        assert!((fixed_pct - 0.50).abs() < 0.01);
    }

    #[test]
    fn test_bb_bonus() {
        let params = AdjustmentParams {
            bb_bonus: 0.40,  // 40% BB bonus (vs default 30%)
            ..Default::default()
        };
        let policies = load_adjusted_inforce(&params).expect("Failed to load");

        let total_bb: f64 = policies.iter().map(|p| p.initial_benefit_base).sum();
        // With 40% bonus: BB = $100M * 1.4 = $140M
        assert!((total_bb - 140_000_000.0).abs() < 100_000.0);
    }
}
