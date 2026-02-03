//! Partial withdrawal (PWD) assumptions
//!
//! Includes non-systematic withdrawals, RMD requirements, and free withdrawal utilization

use crate::policy::QualStatus;

/// RMD (Required Minimum Distribution) table by attained age
#[derive(Debug, Clone)]
pub struct RmdTable {
    /// RMD rates by age (starting from age 73)
    rates: Vec<(u8, f64)>,
}

impl Default for RmdTable {
    fn default() -> Self {
        // From Non-systematic PWDs sheet in Excel
        // Distribution periods and rates starting at age 73
        Self {
            rates: vec![
                (73, 0.0377358491),
                (74, 0.0392156863),
                (75, 0.0406504065),
                (76, 0.0421940928),
                (77, 0.0436681223),
                (78, 0.0454545455),
                (79, 0.0473933649),
                (80, 0.0495049505),
                (81, 0.0515463918),
                (82, 0.0540540541),
                (83, 0.0564971751),
                (84, 0.0595238095),
                (85, 0.0625000000),
                (86, 0.0657894737),
                (87, 0.0694444444),
                (88, 0.0729927007),
                (89, 0.0775193798),
                (90, 0.0819672131),
                (91, 0.0869565217),
                (92, 0.0925925926),
                (93, 0.0990099010),
                (94, 0.1052631579),
                (95, 0.1123595506),
                (96, 0.1190476190),
                (97, 0.1282051282),
                (98, 0.1369863014),
                (99, 0.1470588235),
                (100, 0.1562500000),
                (101, 0.1666666667),
                (102, 0.1785714286),
                (103, 0.1923076923),
                (104, 0.2040816327),
                (105, 0.2173913043),
                (106, 0.2325581395),
                (107, 0.2439024390),
                (108, 0.2564102564),
                (109, 0.2702702703),
                (110, 0.2857142857),
                (111, 0.2941176471),
                (112, 0.3030303030),
                (113, 0.3225806452),
                (114, 0.3333333333),
                (115, 0.3448275862),
                (116, 0.3571428571),
                (117, 0.3703703704),
                (118, 0.4000000000),
                (119, 0.4347826087),
                (120, 0.5000000000),
            ],
        }
    }
}

impl RmdTable {
    /// Create from loaded CSV data
    pub fn from_loaded(rates: &[(u8, f64)]) -> Self {
        Self {
            rates: rates.to_vec(),
        }
    }

    /// Get RMD rate for a given attained age
    /// Returns 0 for ages below RMD start age (73)
    pub fn get_rate(&self, attained_age: u8) -> f64 {
        if attained_age < 73 {
            return 0.0;
        }

        // Find matching age or use last available rate
        for (age, rate) in &self.rates {
            if *age == attained_age {
                return *rate;
            }
        }

        // For ages beyond table, use last rate
        self.rates.last().map(|(_, r)| *r).unwrap_or(0.2)
    }

    /// Get RMD rate applicable for qualified policies
    /// Non-qualified policies have no RMD requirement
    pub fn get_rate_if_qualified(&self, attained_age: u8, qual_status: QualStatus) -> f64 {
        match qual_status {
            QualStatus::Q => self.get_rate(attained_age),
            QualStatus::N => 0.0,
        }
    }
}

/// Free withdrawal utilization by policy year (before income activation)
#[derive(Debug, Clone)]
pub struct FreeWithdrawalUtilization {
    /// Utilization rates by policy year
    rates: Vec<f64>,
}

impl Default for FreeWithdrawalUtilization {
    fn default() -> Self {
        // From Non-systematic PWDs sheet
        // Before income activation, policyholders take a % of free amount
        Self {
            rates: vec![
                0.1, // Year 1: 10%
                0.2, // Year 2: 20%
                0.3, // Year 3: 30%
                0.4, // Year 4+: 40%
            ],
        }
    }
}

impl FreeWithdrawalUtilization {
    /// Create from loaded CSV data
    pub fn from_loaded(rates: &[f64]) -> Self {
        Self {
            rates: rates.to_vec(),
        }
    }

    /// Create from individual year rates
    /// year4_plus is used for year 4 and all subsequent years
    pub fn from_rates(year1: f64, year2: f64, year3: f64, year4_plus: f64) -> Self {
        Self {
            rates: vec![year1, year2, year3, year4_plus],
        }
    }

    /// Get utilization rate for policy year
    pub fn get_rate(&self, policy_year: u32) -> f64 {
        let idx = (policy_year as usize).saturating_sub(1);
        self.rates.get(idx).copied()
            .unwrap_or_else(|| self.rates.last().copied().unwrap_or(0.4))
    }
}

/// Combined PWD assumptions
#[derive(Debug, Clone)]
pub struct PwdAssumptions {
    pub rmd: RmdTable,
    pub free_utilization: FreeWithdrawalUtilization,
}

impl Default for PwdAssumptions {
    fn default() -> Self {
        Self {
            rmd: RmdTable::default(),
            free_utilization: FreeWithdrawalUtilization::default(),
        }
    }
}

impl PwdAssumptions {
    /// Create from loaded CSV assumptions
    pub fn from_loaded(loaded: &super::loader::LoadedAssumptions) -> Self {
        Self {
            rmd: RmdTable::from_loaded(&loaded.rmd_rates),
            free_utilization: FreeWithdrawalUtilization::from_loaded(&loaded.free_withdrawal_util),
        }
    }

    /// Calculate the Free Partial Withdrawal percentage (Excel Column J)
    ///
    /// For qualified policies: MAX(base free %, RMD rate by age)
    /// For non-qualified policies: base free %
    /// In policy year 1: only RMD applies (no free partial withdrawals)
    ///
    /// # Arguments
    /// * `policy_year` - Current policy year
    /// * `attained_age` - Policyholder attained age
    /// * `qual_status` - Qualified or non-qualified
    /// * `free_pct` - Base free withdrawal percentage from ProductFeatures
    ///
    /// # Returns
    /// FPW percentage (e.g., 0.05 for 5%)
    pub fn get_fpw_pct(
        &self,
        policy_year: u32,
        attained_age: u8,
        qual_status: QualStatus,
        free_pct: f64,
    ) -> f64 {
        // In policy year 1, only RMD applies (no free partial withdrawals)
        if policy_year == 1 {
            return match qual_status {
                QualStatus::Q => self.rmd.get_rate(attained_age),
                QualStatus::N => 0.0,
            };
        }

        match qual_status {
            QualStatus::Q => {
                // For qualified: MAX of base free % and RMD rate
                let rmd_rate = self.rmd.get_rate(attained_age);
                free_pct.max(rmd_rate)
            }
            QualStatus::N => {
                // For non-qualified: just base free %
                free_pct
            }
        }
    }

    /// Calculate non-systematic PWD rate for a given month
    ///
    /// # Arguments
    /// * `policy_year` - Current policy year
    /// * `attained_age` - Policyholder attained age
    /// * `qual_status` - Qualified or non-qualified
    /// * `income_activated` - Whether GLWB income has been activated
    /// * `free_pct` - Base free withdrawal percentage from ProductFeatures
    ///
    /// # Returns
    /// Annual PWD rate as a fraction of AV
    pub fn annual_pwd_rate(
        &self,
        policy_year: u32,
        attained_age: u8,
        qual_status: QualStatus,
        income_activated: bool,
        free_pct: f64,
    ) -> f64 {
        if income_activated {
            // After income activation, non-systematic PWDs are minimal
            // Policyholders taking systematic income typically don't take additional PWDs
            return 0.0;
        }

        // Free amount = FPW% (incorporates RMD for qualified contracts)
        let free_rate = self.get_fpw_pct(policy_year, attained_age, qual_status, free_pct);

        // Utilization of the free amount
        let utilization = self.free_utilization.get_rate(policy_year);

        // Annual PWD = free amount × utilization
        free_rate * utilization
    }

    /// Calculate monthly PWD rate
    /// Excel: L = (1-K)*(1-(1-J*util)^(1/12))
    /// Uses actuarial formula to convert annual to monthly
    pub fn monthly_pwd_rate(
        &self,
        policy_year: u32,
        attained_age: u8,
        qual_status: QualStatus,
        income_activated: bool,
        free_pct: f64,
    ) -> f64 {
        let annual = self.annual_pwd_rate(policy_year, attained_age, qual_status, income_activated, free_pct);

        // Convert to monthly using actuarial formula: 1 - (1 - annual)^(1/12)
        1.0 - (1.0 - annual).powf(1.0 / 12.0)
    }

    /// Calculate monthly PWD rate with policy year adjustment
    /// Excel: FPW% = 0 for policy year 1 (no withdrawals in first year)
    /// Excel: L = (1-K)*(1-(1-J*util)^(1/12))
    pub fn monthly_pwd_rate_adjusted(
        &self,
        policy_year: u32,
        _month_in_policy_year: u32,
        attained_age: u8,
        qual_status: QualStatus,
        income_activated: bool,
        free_pct: f64,
    ) -> f64 {
        // Excel sets FPW% = 0 for entire policy year 1
        if policy_year == 1 {
            return 0.0;
        }

        let annual = self.annual_pwd_rate(policy_year, attained_age, qual_status, income_activated, free_pct);

        // Convert to monthly using actuarial formula: 1 - (1 - annual)^(1/12)
        1.0 - (1.0 - annual).powf(1.0 / 12.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rmd_rates() {
        let rmd = RmdTable::default();

        // Below RMD age
        assert_eq!(rmd.get_rate(70), 0.0);

        // At RMD ages
        assert!((rmd.get_rate(73) - 0.0377).abs() < 0.001);
        assert!((rmd.get_rate(77) - 0.0437).abs() < 0.001);
        assert!((rmd.get_rate(85) - 0.0625).abs() < 0.001);
    }

    #[test]
    fn test_free_utilization() {
        let util = FreeWithdrawalUtilization::default();

        assert_eq!(util.get_rate(1), 0.1);
        assert_eq!(util.get_rate(2), 0.2);
        assert_eq!(util.get_rate(3), 0.3);
        assert_eq!(util.get_rate(4), 0.4);
        assert_eq!(util.get_rate(10), 0.4);
    }

    #[test]
    fn test_pwd_assumptions() {
        let pwd = PwdAssumptions::default();
        let free_pct = 0.05; // 5% free withdrawal from ProductFeatures

        // Year 1, age 60, non-qualified, not activated - FPW% is 0 in year 1 (no free, no RMD)
        let rate = pwd.annual_pwd_rate(1, 60, QualStatus::N, false, free_pct);
        assert_eq!(rate, 0.0);

        // Year 1, age 75, qualified - RMD rate at 75 = 0.0407
        // Only RMD applies in year 1 for qualified
        // Annual rate = 4.07% × 10% utilization = 0.407%
        let rate_y1_q = pwd.annual_pwd_rate(1, 75, QualStatus::Q, false, free_pct);
        let expected_y1_q = 0.0406504065 * 0.1;  // RMD(75) × year 1 utilization
        assert!((rate_y1_q - expected_y1_q).abs() < 0.001);

        // Year 2, age 61, non-qualified - 5% free × 20% utilization = 1%
        let rate_y2 = pwd.annual_pwd_rate(2, 61, QualStatus::N, false, free_pct);
        assert!((rate_y2 - 0.01).abs() < 0.001);

        // Year 4, age 77, qualified, not activated
        // RMD rate at 77 = 0.0437, which is < 5% free, so uses 5% free
        // Annual rate = 5% * 40% utilization = 2%
        let rate_q = pwd.annual_pwd_rate(4, 77, QualStatus::Q, false, free_pct);
        assert!((rate_q - 0.02).abs() < 0.001); // 5% free × 40% utilization

        // Year 4, age 85, qualified - RMD = 6.25% > 5% free
        // Annual rate = 6.25% * 40% = 2.5%
        let rate_rmd = pwd.annual_pwd_rate(4, 85, QualStatus::Q, false, free_pct);
        assert!((rate_rmd - 0.025).abs() < 0.001);

        // After income activation - no PWDs
        let rate_activated = pwd.annual_pwd_rate(4, 77, QualStatus::Q, true, free_pct);
        assert_eq!(rate_activated, 0.0);

        // Test monthly rate conversion
        // Annual 2% → monthly = 1 - (1-0.02)^(1/12) ≈ 0.00168
        let monthly = pwd.monthly_pwd_rate(4, 77, QualStatus::Q, false, free_pct);
        let expected_monthly = 1.0 - (1.0 - 0.02_f64).powf(1.0 / 12.0);
        assert!((monthly - expected_monthly).abs() < 0.0001);
    }

    #[test]
    fn test_custom_utilization_rates() {
        let pwd = PwdAssumptions {
            rmd: RmdTable::default(),
            free_utilization: FreeWithdrawalUtilization::from_rates(0.065, 0.13, 0.195, 0.26),
        };
        let free_pct = 0.10; // 10% free withdrawal

        // Year 1, age 60, non-qualified - 0% (no free, no RMD below 73)
        let rate_y1_nq = pwd.annual_pwd_rate(1, 60, QualStatus::N, false, free_pct);
        assert_eq!(rate_y1_nq, 0.0);

        // Year 2, non-qualified - 10% × 13% = 1.3%
        let rate_y2 = pwd.annual_pwd_rate(2, 61, QualStatus::N, false, free_pct);
        assert!((rate_y2 - 0.013).abs() < 0.001);

        // Year 3, non-qualified - 10% × 19.5% = 1.95%
        let rate_y3 = pwd.annual_pwd_rate(3, 62, QualStatus::N, false, free_pct);
        assert!((rate_y3 - 0.0195).abs() < 0.001);

        // Year 4+, non-qualified - 10% × 26% = 2.6%
        let rate_y4 = pwd.annual_pwd_rate(4, 63, QualStatus::N, false, free_pct);
        assert!((rate_y4 - 0.026).abs() < 0.001);
    }
}
