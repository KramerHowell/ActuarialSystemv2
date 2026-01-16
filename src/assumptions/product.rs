//! Product features including surrender charges, payout factors, and rider terms

use std::collections::HashMap;

/// Surrender charge schedule by policy year
#[derive(Debug, Clone)]
pub struct SurrenderChargeSchedule {
    /// Surrender charge rates by policy year (1-indexed)
    charges: Vec<f64>,
}

impl SurrenderChargeSchedule {
    /// Create from loaded CSV data
    pub fn from_loaded(charges: &[f64]) -> Self {
        Self {
            charges: charges.to_vec(),
        }
    }

    /// Create default 10-year surrender charge schedule
    pub fn default_10_year() -> Self {
        Self {
            // Year 1-10 charges, year 11+ is 0
            charges: vec![
                0.09, // Year 1
                0.09, // Year 2
                0.08, // Year 3
                0.07, // Year 4
                0.06, // Year 5
                0.05, // Year 6
                0.04, // Year 7
                0.03, // Year 8
                0.02, // Year 9
                0.01, // Year 10
            ],
        }
    }

    /// Get surrender charge rate for a given policy year
    pub fn get_rate(&self, policy_year: u32) -> f64 {
        if policy_year == 0 {
            return self.charges.first().copied().unwrap_or(0.0);
        }
        let idx = (policy_year as usize).saturating_sub(1);
        self.charges.get(idx).copied().unwrap_or(0.0)
    }

    /// Check if still in surrender charge period
    pub fn in_sc_period(&self, policy_year: u32) -> bool {
        self.get_rate(policy_year) > 0.0
    }

    /// Get the total SC period length in years
    pub fn sc_period_years(&self) -> u32 {
        self.charges.len() as u32
    }
}

/// GLWB payout factors by attained age
#[derive(Debug, Clone)]
pub struct PayoutFactors {
    /// Single life payout factors by age band
    single_life: HashMap<(u8, u8), f64>,
    /// Joint life payout factors by age band (optional)
    joint_life: Option<HashMap<(u8, u8), f64>>,
}

impl PayoutFactors {
    /// Create from loaded CSV data (HashMap<age, factor>)
    pub fn from_loaded(factors: &std::collections::HashMap<u8, f64>) -> Self {
        // Convert direct age->factor mapping to age bands
        // For now, store as single-year bands
        let mut single_life = HashMap::new();
        for (&age, &factor) in factors {
            single_life.insert((age, age), factor);
        }
        Self {
            single_life,
            joint_life: None,
        }
    }

    /// Create default payout factors from Product features sheet
    /// Uses per-age factors from Excel (not banded)
    pub fn default() -> Self {
        let mut single_life = HashMap::new();

        // Per-age payout factors from Excel Product features sheet
        // Ages 50-55 use band rate
        single_life.insert((50, 55), 0.046);
        // Ages 56+ use per-year rates
        single_life.insert((56, 56), 0.0475);
        single_life.insert((57, 57), 0.049);
        single_life.insert((58, 58), 0.0505);
        single_life.insert((59, 59), 0.052);
        single_life.insert((60, 60), 0.0535);
        single_life.insert((61, 61), 0.055);
        single_life.insert((62, 62), 0.0565);
        single_life.insert((63, 63), 0.058);
        single_life.insert((64, 64), 0.0595);
        single_life.insert((65, 65), 0.0605);
        single_life.insert((66, 66), 0.061);
        single_life.insert((67, 67), 0.062);
        single_life.insert((68, 68), 0.0625);
        single_life.insert((69, 69), 0.0635);
        single_life.insert((70, 70), 0.0645);
        single_life.insert((71, 71), 0.0655);
        single_life.insert((72, 72), 0.0665);
        single_life.insert((73, 73), 0.0675);
        single_life.insert((74, 74), 0.069);
        single_life.insert((75, 75), 0.0705);
        single_life.insert((76, 76), 0.0725);
        single_life.insert((77, 77), 0.0745);
        single_life.insert((78, 78), 0.0765);
        single_life.insert((79, 79), 0.0785);
        single_life.insert((80, 80), 0.0795);
        single_life.insert((81, 81), 0.0805);
        single_life.insert((82, 82), 0.0815);
        single_life.insert((83, 83), 0.0825);
        single_life.insert((84, 84), 0.0835);
        single_life.insert((85, 85), 0.0845);
        single_life.insert((86, 86), 0.0855);
        single_life.insert((87, 87), 0.0865);
        single_life.insert((88, 88), 0.0875);
        single_life.insert((89, 89), 0.0885);
        single_life.insert((90, 120), 0.0895);  // 90+ use max rate

        Self {
            single_life,
            joint_life: None,
        }
    }

    /// Get single life payout factor for attained age
    pub fn get_single_life(&self, attained_age: u8) -> f64 {
        for ((min_age, max_age), factor) in &self.single_life {
            if attained_age >= *min_age && attained_age <= *max_age {
                return *factor;
            }
        }
        // Default to highest age band if beyond range
        0.090
    }

    /// Get joint life payout factor for attained age (if available)
    pub fn get_joint_life(&self, attained_age: u8) -> Option<f64> {
        self.joint_life.as_ref().and_then(|jl| {
            for ((min_age, max_age), factor) in jl {
                if attained_age >= *min_age && attained_age <= *max_age {
                    return Some(*factor);
                }
            }
            None
        })
    }
}

/// GLWB rider features
#[derive(Debug, Clone)]
pub struct GlwbFeatures {
    /// Minimum age for income activation
    pub min_activation_age: u8,

    /// Bonus percentage applied to initial premium for benefit base
    pub bonus_rate: f64,

    /// Annual rollup rate for benefit base
    pub rollup_rate: f64,

    /// Maximum years for rollup
    pub rollup_years: u8,

    /// Is rollup simple or compound interest
    pub simple_rollup: bool,

    /// Rider charge rate before income activation (annual)
    pub pre_activation_charge: f64,

    /// Rider charge rate after income activation (annual)
    pub post_activation_charge: f64,

    /// Payout factors by age
    pub payout_factors: PayoutFactors,
}

impl Default for GlwbFeatures {
    fn default() -> Self {
        Self {
            min_activation_age: 50,
            bonus_rate: 0.30,           // 30% bonus
            rollup_rate: 0.10,          // 10% annual rollup
            rollup_years: 10,           // 10 years of rollup
            simple_rollup: true,        // Simple interest
            pre_activation_charge: 0.005,  // 0.5% per annum
            post_activation_charge: 0.015, // 1.5% per annum
            payout_factors: PayoutFactors::default(),
        }
    }
}

impl GlwbFeatures {
    /// Calculate monthly rider charge rate based on activation status
    pub fn monthly_rider_charge(&self, income_activated: bool) -> f64 {
        let annual_rate = if income_activated {
            self.post_activation_charge
        } else {
            self.pre_activation_charge
        };
        annual_rate / 12.0
    }

    /// Calculate monthly rollup factor for benefit base
    /// Returns the factor to multiply benefit base by (> 1.0 means growth)
    pub fn monthly_rollup_factor(&self, policy_year: u32, income_activated: bool) -> f64 {
        // No rollup after income activation or beyond rollup period
        if income_activated || policy_year > self.rollup_years as u32 {
            return 1.0;
        }

        if self.simple_rollup {
            // Simple interest: add (rollup_rate / 12) of INITIAL benefit base each month
            // This is handled differently - return the monthly addition rate
            // For simple rollup, we track the monthly increment separately
            1.0 + self.rollup_rate / 12.0
        } else {
            // Compound interest: multiply by (1 + rate)^(1/12)
            (1.0 + self.rollup_rate).powf(1.0 / 12.0)
        }
    }

    /// Calculate maximum withdrawal amount for the year
    pub fn max_annual_withdrawal(&self, benefit_base: f64, attained_age: u8) -> f64 {
        let payout_rate = self.payout_factors.get_single_life(attained_age);
        benefit_base * payout_rate
    }
}

/// Base product features (non-rider)
#[derive(Debug, Clone)]
pub struct BaseProductFeatures {
    /// Surrender charge schedule
    pub surrender_charges: SurrenderChargeSchedule,

    /// Free partial withdrawal percentage per year
    pub free_withdrawal_pct: f64,

    /// Minimum premium
    pub min_premium: f64,

    /// Maximum premium
    pub max_premium: f64,

    /// Minimum issue age
    pub min_issue_age: u8,

    /// Maximum issue age
    pub max_issue_age: u8,

    /// Annual expense per policy (dollars) - set to 0 if using expense_rate_of_av
    pub annual_expense_per_policy: f64,

    /// Annual expense rate as percentage of EOP AV (e.g., 0.0025 = 0.25%)
    /// Applied monthly as rate/12 * EOP_AV
    pub expense_rate_of_av: f64,

    /// First year commission rate (as decimal, e.g., 0.05 = 5%) - DEPRECATED, use CommissionAssumptions
    pub first_year_commission_rate: f64,
}

impl Default for BaseProductFeatures {
    fn default() -> Self {
        Self {
            surrender_charges: SurrenderChargeSchedule::default_10_year(),
            free_withdrawal_pct: 0.05,           // 5% free withdrawal
            min_premium: 25_000.0,
            max_premium: 1_000_000.0,
            min_issue_age: 40,
            max_issue_age: 80,
            annual_expense_per_policy: 0.0,      // $0 - using expense_rate_of_av instead
            expense_rate_of_av: 0.0025,          // 0.25% of EOP AV annually
            first_year_commission_rate: 0.05,   // DEPRECATED - 5% first year commission
        }
    }
}

/// Commission assumptions with age-based rates and chargeback schedule
#[derive(Debug, Clone)]
pub struct CommissionAssumptions {
    /// Age threshold: at or below uses "young" rates, above uses "old" rates
    pub age_threshold: u8,

    /// Agent commission rate for ages <= threshold (e.g., 0.07 = 7%)
    pub agent_rate_young: f64,
    /// Agent commission rate for ages > threshold (e.g., 0.045 = 4.5%)
    pub agent_rate_old: f64,

    /// IMO gross override rate before conversion for ages <= threshold (e.g., 0.036 = 3.6%)
    pub imo_gross_rate_young: f64,
    /// IMO gross override rate before conversion for ages > threshold (e.g., 0.014 = 1.4%)
    pub imo_gross_rate_old: f64,
    /// Wholesaler gross override rate before conversion for ages <= threshold (e.g., 0.006 = 0.6%)
    pub wholesaler_gross_rate_young: f64,
    /// Wholesaler gross override rate before conversion for ages > threshold (e.g., 0.003 = 0.3%)
    pub wholesaler_gross_rate_old: f64,

    /// IMO equity conversion rate (e.g., 0.25 = 25%)
    pub imo_conversion_rate: f64,
    /// Wholesaler conversion rate (e.g., 0.40 = 40%)
    pub wholesaler_conversion_rate: f64,

    /// Month 13 bonus rate on BOP AV for young ages (e.g., 0.005 = 0.5%)
    pub bonus_rate_young: f64,

    /// Months with 100% chargeback (e.g., 6 = months 1-6)
    pub chargeback_months_full: u32,
    /// Months with 50% chargeback (e.g., 12 = months 7-12)
    pub chargeback_months_half: u32,
}

impl Default for CommissionAssumptions {
    fn default() -> Self {
        Self {
            age_threshold: 75,
            agent_rate_young: 0.07,              // 7%
            agent_rate_old: 0.045,               // 4.5%
            imo_gross_rate_young: 0.036,         // 3.6%
            imo_gross_rate_old: 0.017 * 6.0 / 7.0,   // ~1.46%
            wholesaler_gross_rate_young: 0.006,  // 0.6%
            wholesaler_gross_rate_old: 0.017 / 7.0,    // ~0.24%
            imo_conversion_rate: 0.25,           // 25%
            wholesaler_conversion_rate: 0.40,    // 40%
            bonus_rate_young: 0.005,             // 0.5%
            chargeback_months_full: 6,
            chargeback_months_half: 12,
        }
    }
}

impl CommissionAssumptions {
    /// Get agent commission rate based on issue age
    pub fn agent_rate(&self, issue_age: u8) -> f64 {
        if issue_age <= self.age_threshold {
            self.agent_rate_young
        } else {
            self.agent_rate_old
        }
    }

    /// Get bonus rate based on issue age
    /// Older ages: bonus_rate_young * (agent_rate_old / agent_rate_young)
    pub fn bonus_rate(&self, issue_age: u8) -> f64 {
        if issue_age <= self.age_threshold {
            self.bonus_rate_young
        } else {
            self.bonus_rate_young * self.agent_rate_old / self.agent_rate_young
        }
    }

    /// Get chargeback factor based on projection month
    /// 100% for months 1-6, 50% for months 7-12, 0% after
    pub fn chargeback_factor(&self, projection_month: u32, policy_year: u32) -> f64 {
        if policy_year > 1 {
            0.0
        } else if projection_month <= self.chargeback_months_full {
            1.0
        } else if projection_month <= self.chargeback_months_half {
            0.5
        } else {
            0.0
        }
    }

    /// Calculate all commission components for a given premium and issue age
    /// Returns (agent, imo_net, imo_conversion, wholesaler_net, wholesaler_conversion)
    pub fn calculate_commissions(&self, premium: f64, issue_age: u8) -> (f64, f64, f64, f64, f64) {
        let agent = premium * self.agent_rate(issue_age);

        let (imo_gross, wholesaler_gross) = if issue_age <= self.age_threshold {
            // Ages 0-75: use young rates
            (premium * self.imo_gross_rate_young, premium * self.wholesaler_gross_rate_young)
        } else {
            // Ages 76+: use old rates
            (premium * self.imo_gross_rate_old, premium * self.wholesaler_gross_rate_old)
        };

        let imo_net = imo_gross * (1.0 - self.imo_conversion_rate);
        let imo_conversion = imo_gross * self.imo_conversion_rate;
        let wholesaler_net = wholesaler_gross * (1.0 - self.wholesaler_conversion_rate);
        let wholesaler_conversion = wholesaler_gross * self.wholesaler_conversion_rate;

        (agent, imo_net, imo_conversion, wholesaler_net, wholesaler_conversion)
    }
}

/// Combined product features
#[derive(Debug, Clone)]
pub struct ProductFeatures {
    pub base: BaseProductFeatures,
    pub glwb: GlwbFeatures,
    pub commissions: CommissionAssumptions,
}

impl Default for ProductFeatures {
    fn default() -> Self {
        Self {
            base: BaseProductFeatures::default(),
            glwb: GlwbFeatures::default(),
            commissions: CommissionAssumptions::default(),
        }
    }
}

impl ProductFeatures {
    /// Create from loaded CSV assumptions
    pub fn from_loaded(loaded: &super::loader::LoadedAssumptions) -> Self {
        let mut features = Self::default();
        features.base.surrender_charges = SurrenderChargeSchedule::from_loaded(&loaded.surrender_charges);
        features.glwb.payout_factors = PayoutFactors::from_loaded(&loaded.payout_factors);
        features
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surrender_charges() {
        let sc = SurrenderChargeSchedule::default_10_year();

        assert_eq!(sc.get_rate(1), 0.09);
        assert_eq!(sc.get_rate(5), 0.06);
        assert_eq!(sc.get_rate(10), 0.01);
        assert_eq!(sc.get_rate(11), 0.0);
        assert_eq!(sc.get_rate(20), 0.0);
    }

    #[test]
    fn test_payout_factors() {
        let pf = PayoutFactors::default();

        // Per-age factors from Excel Product features sheet
        assert_eq!(pf.get_single_life(52), 0.046);   // Band 50-55
        assert_eq!(pf.get_single_life(61), 0.055);   // Age 61
        assert_eq!(pf.get_single_life(64), 0.0595);  // Age 64
        assert_eq!(pf.get_single_life(65), 0.0605);  // Age 65
        assert_eq!(pf.get_single_life(77), 0.0745);  // Age 77
        assert_eq!(pf.get_single_life(90), 0.0895);  // Age 90+
    }

    #[test]
    fn test_glwb_rollup() {
        let glwb = GlwbFeatures::default();

        // During rollup period, not activated
        let factor = glwb.monthly_rollup_factor(1, false);
        assert!((factor - (1.0 + 0.10 / 12.0)).abs() < 1e-10);

        // After income activation - no rollup
        assert_eq!(glwb.monthly_rollup_factor(1, true), 1.0);

        // After rollup period - no rollup
        assert_eq!(glwb.monthly_rollup_factor(11, false), 1.0);
    }
}
