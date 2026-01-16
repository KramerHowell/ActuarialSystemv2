//! Dynamic policy generator for pricing inforce
//!
//! Generates policies on-the-fly based on configurable parameters:
//! - Fixed/Indexed split
//! - Gender split adjustment
//! - Qual status split adjustment
//! - Benefit base bonus

use super::{Policy, QualStatus, Gender, CreditingStrategy, RollupType, BenefitBaseBucket};
use serde::{Deserialize, Serialize};

/// Parameters for generating the inforce
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InforceParams {
    /// Percentage of premium allocated to Fixed strategy (0.0 to 1.0)
    /// Default: 0.25 (25%)
    #[serde(default = "default_fixed_pct")]
    pub fixed_pct: f64,

    /// Multiplier for male policies (1.0 = no change)
    /// Adjusts the male portion relative to base demographics
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

    /// Benefit base bonus (0.0 = no bonus, 0.10 = 10% bonus)
    #[serde(default)]
    pub bonus: f64,

    /// Target total premium (default: $100M)
    #[serde(default = "default_target_premium")]
    pub target_premium: f64,
}

fn default_fixed_pct() -> f64 { 0.25 }
fn default_one() -> f64 { 1.0 }
fn default_target_premium() -> f64 { 100_000_000.0 }

impl Default for InforceParams {
    fn default() -> Self {
        Self {
            fixed_pct: 0.25,
            male_mult: 1.0,
            female_mult: 1.0,
            qual_mult: 1.0,
            nonqual_mult: 1.0,
            bonus: 0.0,
            target_premium: 100_000_000.0,
        }
    }
}

/// Base demographic cell from FIA_inforce aggregation
#[derive(Debug, Clone)]
struct DemographicCell {
    qual_status: QualStatus,
    issue_age: u8,
    gender: Gender,
    benefit_base_bucket: BenefitBaseBucket,
    /// Base weight from original demographics (InitialPremium before scaling)
    base_weight: f64,
    /// Ratio of InitialBB to InitialPremium (typically 1.3)
    bb_to_premium_ratio: f64,
    /// Base InitialPols per unit premium
    pols_per_premium: f64,
}

/// Utilization rate by GLWB start duration
#[derive(Debug, Clone)]
struct UtilizationRate {
    glwb_start_year: u32,
    /// Incremental utilization percentage for this duration
    percentage: f64,
}

/// Pre-computed base template for generating policies
pub struct InforceTemplate {
    cells: Vec<DemographicCell>,
    utilization: Vec<UtilizationRate>,
}

impl InforceTemplate {
    /// Create the template from hardcoded base data
    /// This represents the aggregated FIA_inforce demographics and GLWB utilization
    pub fn new() -> Self {
        let cells = build_demographic_cells();
        let utilization = build_utilization_rates();
        Self { cells, utilization }
    }

    /// Generate policies based on parameters
    pub fn generate(&self, params: &InforceParams) -> Vec<Policy> {
        let indexed_pct = 1.0 - params.fixed_pct;
        let mut policies = Vec::with_capacity(2800);
        let mut policy_id = 1u32;

        // First pass: calculate total weighted premium for scaling
        let mut total_weighted_premium = 0.0;
        for cell in &self.cells {
            let gender_mult = match cell.gender {
                Gender::Male => params.male_mult,
                Gender::Female => params.female_mult,
            };
            let qual_mult = match cell.qual_status {
                QualStatus::Q => params.qual_mult,
                QualStatus::N => params.nonqual_mult,
            };
            let cell_mult = gender_mult * qual_mult;

            for util in &self.utilization {
                // Both Fixed and Indexed contribute
                let fixed_weight = cell.base_weight * cell_mult * util.percentage * params.fixed_pct;
                let indexed_weight = cell.base_weight * cell_mult * util.percentage * indexed_pct;
                total_weighted_premium += fixed_weight + indexed_weight;
            }
        }

        // Scale factor to hit target premium
        let scale = if total_weighted_premium > 0.0 {
            params.target_premium / total_weighted_premium
        } else {
            1.0
        };

        // Second pass: generate policies
        for cell in &self.cells {
            let gender_mult = match cell.gender {
                Gender::Male => params.male_mult,
                Gender::Female => params.female_mult,
            };
            let qual_mult = match cell.qual_status {
                QualStatus::Q => params.qual_mult,
                QualStatus::N => params.nonqual_mult,
            };
            let cell_mult = gender_mult * qual_mult;

            for util in &self.utilization {
                // Generate Fixed policy
                if params.fixed_pct > 0.0 {
                    let percentage = util.percentage * params.fixed_pct * cell_mult;
                    let initial_premium = cell.base_weight * percentage * scale;
                    let initial_bb = initial_premium * cell.bb_to_premium_ratio * (1.0 + params.bonus);
                    let initial_pols = initial_premium * cell.pols_per_premium;

                    policies.push(Policy {
                        policy_id,
                        qual_status: cell.qual_status,
                        issue_age: cell.issue_age,
                        gender: cell.gender,
                        initial_benefit_base: initial_bb,
                        initial_pols,
                        initial_premium,
                        benefit_base_bucket: cell.benefit_base_bucket,
                        percentage,
                        crediting_strategy: CreditingStrategy::Fixed,
                        sc_period: 10,
                        val_rate: 0.0475,
                        mgir: 0.01,
                        bonus: params.bonus,
                        rollup_type: RollupType::Simple,
                        duration_months: 0,
                        income_activated: false,
                        glwb_start_year: util.glwb_start_year,
                        current_av: None,
                        current_benefit_base: None,
                    });
                    policy_id += 1;
                }

                // Generate Indexed policy
                if indexed_pct > 0.0 {
                    let percentage = util.percentage * indexed_pct * cell_mult;
                    let initial_premium = cell.base_weight * percentage * scale;
                    let initial_bb = initial_premium * cell.bb_to_premium_ratio * (1.0 + params.bonus);
                    let initial_pols = initial_premium * cell.pols_per_premium;

                    policies.push(Policy {
                        policy_id,
                        qual_status: cell.qual_status,
                        issue_age: cell.issue_age,
                        gender: cell.gender,
                        initial_benefit_base: initial_bb,
                        initial_pols,
                        initial_premium,
                        benefit_base_bucket: cell.benefit_base_bucket,
                        percentage,
                        crediting_strategy: CreditingStrategy::Indexed,
                        sc_period: 10,
                        val_rate: 0.0475,
                        mgir: 0.01,
                        bonus: params.bonus,
                        rollup_type: RollupType::Simple,
                        duration_months: 0,
                        income_activated: false,
                        glwb_start_year: util.glwb_start_year,
                        current_av: None,
                        current_benefit_base: None,
                    });
                    policy_id += 1;
                }
            }
        }

        policies
    }
}

impl Default for InforceTemplate {
    fn default() -> Self {
        Self::new()
    }
}

/// Build demographic cells from aggregated FIA_inforce data
/// These represent the 100 unique (QualStatus, IssueAge, Gender, BB_Bucket) combinations
fn build_demographic_cells() -> Vec<DemographicCell> {
    // Data extracted from FIA_inforce aggregation
    // Format: (qual_status, issue_age, gender, bb_bucket, base_weight, bb_ratio, pols_per_premium)
    // base_weight is relative premium weight within the demographic

    let mut cells = Vec::with_capacity(100);

    // Issue ages: 57, 62, 67, 72, 77
    let issue_ages = [57u8, 62, 67, 72, 77];
    let genders = [Gender::Female, Gender::Male];
    let qual_statuses = [QualStatus::N, QualStatus::Q];
    let bb_buckets = [
        BenefitBaseBucket::Under50k,
        BenefitBaseBucket::From50kTo100k,
        BenefitBaseBucket::From100kTo200k,
        BenefitBaseBucket::From200kTo500k,
        BenefitBaseBucket::Over500k,
    ];

    // Actual weights extracted from pricing_inforce.csv
    // Format: (qual_status, issue_age, gender, bb_bucket, weight, pols_per_premium)
    let weights: &[(&str, u8, &str, &str, f64, f64)] = &[
        // Non-Qualified
        ("N", 57, "Female", "Under50k", 0.0009088384, 0.0000465703),
        ("N", 57, "Female", "From50kTo100k", 0.0016700474, 0.0000198933),
        ("N", 57, "Female", "From100kTo200k", 0.0037518498, 0.0000104320),
        ("N", 57, "Female", "From200kTo500k", 0.0052106508, 0.0000046509),
        ("N", 57, "Female", "Over500k", 0.0019357818, 0.0000021159),
        ("N", 57, "Male", "Under50k", 0.0005666217, 0.0000467860),
        ("N", 57, "Male", "From50kTo100k", 0.0013776594, 0.0000200686),
        ("N", 57, "Male", "From100kTo200k", 0.0033360405, 0.0000105726),
        ("N", 57, "Male", "From200kTo500k", 0.0045829830, 0.0000047417),
        ("N", 57, "Male", "Over500k", 0.0039822665, 0.0000020857),

        ("N", 62, "Female", "Under50k", 0.0012627984, 0.0000432474),
        ("N", 62, "Female", "From50kTo100k", 0.0030721965, 0.0000189245),
        ("N", 62, "Female", "From100kTo200k", 0.0075322688, 0.0000097278),
        ("N", 62, "Female", "From200kTo500k", 0.0099602441, 0.0000046720),
        ("N", 62, "Female", "Over500k", 0.0042468426, 0.0000019289),
        ("N", 62, "Male", "Under50k", 0.0011891259, 0.0000450657),
        ("N", 62, "Male", "From50kTo100k", 0.0032639381, 0.0000197300),
        ("N", 62, "Male", "From100kTo200k", 0.0069055586, 0.0000101987),
        ("N", 62, "Male", "From200kTo500k", 0.0124235655, 0.0000045699),
        ("N", 62, "Male", "Over500k", 0.0077321023, 0.0000017069),

        ("N", 67, "Female", "Under50k", 0.0015433994, 0.0000420931),
        ("N", 67, "Female", "From50kTo100k", 0.0038432176, 0.0000188581),
        ("N", 67, "Female", "From100kTo200k", 0.0093190765, 0.0000099992),
        ("N", 67, "Female", "From200kTo500k", 0.0122689747, 0.0000045626),
        ("N", 67, "Female", "Over500k", 0.0049711141, 0.0000018081),
        ("N", 67, "Male", "Under50k", 0.0013496194, 0.0000426572),
        ("N", 67, "Male", "From50kTo100k", 0.0040339383, 0.0000196870),
        ("N", 67, "Male", "From100kTo200k", 0.0092925930, 0.0000099297),
        ("N", 67, "Male", "From200kTo500k", 0.0123062309, 0.0000046135),
        ("N", 67, "Male", "Over500k", 0.0097612917, 0.0000017134),

        ("N", 72, "Female", "Under50k", 0.0014501436, 0.0000444862),
        ("N", 72, "Female", "From50kTo100k", 0.0039342324, 0.0000191159),
        ("N", 72, "Female", "From100kTo200k", 0.0085403775, 0.0000100583),
        ("N", 72, "Female", "From200kTo500k", 0.0121846274, 0.0000046875),
        ("N", 72, "Female", "Over500k", 0.0047865795, 0.0000019016),
        ("N", 72, "Male", "Under50k", 0.0013222255, 0.0000425944),
        ("N", 72, "Male", "From50kTo100k", 0.0033266226, 0.0000194608),
        ("N", 72, "Male", "From100kTo200k", 0.0087869354, 0.0000100350),
        ("N", 72, "Male", "From200kTo500k", 0.0141311099, 0.0000045733),
        ("N", 72, "Male", "Over500k", 0.0061983865, 0.0000018723),

        ("N", 77, "Female", "Under50k", 0.0013038629, 0.0000436306),
        ("N", 77, "Female", "From50kTo100k", 0.0034898805, 0.0000189743),
        ("N", 77, "Female", "From100kTo200k", 0.0075870913, 0.0000100174),
        ("N", 77, "Female", "From200kTo500k", 0.0098199849, 0.0000046345),
        ("N", 77, "Female", "Over500k", 0.0034597776, 0.0000020718),
        ("N", 77, "Male", "Under50k", 0.0009282650, 0.0000413058),
        ("N", 77, "Male", "From50kTo100k", 0.0024116374, 0.0000192015),
        ("N", 77, "Male", "From100kTo200k", 0.0053150746, 0.0000099326),
        ("N", 77, "Male", "From200kTo500k", 0.0069946584, 0.0000046521),
        ("N", 77, "Male", "Over500k", 0.0030539619, 0.0000020118),

        // Qualified
        ("Q", 57, "Female", "Under50k", 0.0019436203, 0.0000422063),
        ("Q", 57, "Female", "From50kTo100k", 0.0049867748, 0.0000189598),
        ("Q", 57, "Female", "From100kTo200k", 0.0137556748, 0.0000098593),
        ("Q", 57, "Female", "From200kTo500k", 0.0200372920, 0.0000047868),
        ("Q", 57, "Female", "Over500k", 0.0073878261, 0.0000020483),
        ("Q", 57, "Male", "Under50k", 0.0013065432, 0.0000439765),
        ("Q", 57, "Male", "From50kTo100k", 0.0044466984, 0.0000189854),
        ("Q", 57, "Male", "From100kTo200k", 0.0150321249, 0.0000098320),
        ("Q", 57, "Male", "From200kTo500k", 0.0280989220, 0.0000046160),
        ("Q", 57, "Male", "Over500k", 0.0102441597, 0.0000019658),

        ("Q", 62, "Female", "Under50k", 0.0031516706, 0.0000411184),
        ("Q", 62, "Female", "From50kTo100k", 0.0109928723, 0.0000185990),
        ("Q", 62, "Female", "From100kTo200k", 0.0298317213, 0.0000095654),
        ("Q", 62, "Female", "From200kTo500k", 0.0463375999, 0.0000046628),
        ("Q", 62, "Female", "Over500k", 0.0124657093, 0.0000020171),
        ("Q", 62, "Male", "Under50k", 0.0025804785, 0.0000417545),
        ("Q", 62, "Male", "From50kTo100k", 0.0097172177, 0.0000193429),
        ("Q", 62, "Male", "From100kTo200k", 0.0347338258, 0.0000098434),
        ("Q", 62, "Male", "From200kTo500k", 0.0725250764, 0.0000046028),
        ("Q", 62, "Male", "Over500k", 0.0301196740, 0.0000019265),

        ("Q", 67, "Female", "Under50k", 0.0034949566, 0.0000391956),
        ("Q", 67, "Female", "From50kTo100k", 0.0097816610, 0.0000184012),
        ("Q", 67, "Female", "From100kTo200k", 0.0243278247, 0.0000097091),
        ("Q", 67, "Female", "From200kTo500k", 0.0378745362, 0.0000046172),
        ("Q", 67, "Female", "Over500k", 0.0116479219, 0.0000020708),
        ("Q", 67, "Male", "Under50k", 0.0024258913, 0.0000408038),
        ("Q", 67, "Male", "From50kTo100k", 0.0098179558, 0.0000183680),
        ("Q", 67, "Male", "From100kTo200k", 0.0306969894, 0.0000098999),
        ("Q", 67, "Male", "From200kTo500k", 0.0644354866, 0.0000045697),
        ("Q", 67, "Male", "Over500k", 0.0262050715, 0.0000020189),

        ("Q", 72, "Female", "Under50k", 0.0020744933, 0.0000397630),
        ("Q", 72, "Female", "From50kTo100k", 0.0062453015, 0.0000181997),
        ("Q", 72, "Female", "From100kTo200k", 0.0138438925, 0.0000093527),
        ("Q", 72, "Female", "From200kTo500k", 0.0209000022, 0.0000044367),
        ("Q", 72, "Female", "Over500k", 0.0057075570, 0.0000018738),
        ("Q", 72, "Male", "Under50k", 0.0014879832, 0.0000402963),
        ("Q", 72, "Male", "From50kTo100k", 0.0052891014, 0.0000176825),
        ("Q", 72, "Male", "From100kTo200k", 0.0153710041, 0.0000095486),
        ("Q", 72, "Male", "From200kTo500k", 0.0316566493, 0.0000044567),
        ("Q", 72, "Male", "Over500k", 0.0130669421, 0.0000018198),

        ("Q", 77, "Female", "Under50k", 0.0009944025, 0.0000408469),
        ("Q", 77, "Female", "From50kTo100k", 0.0028450231, 0.0000180761),
        ("Q", 77, "Female", "From100kTo200k", 0.0057428956, 0.0000095096),
        ("Q", 77, "Female", "From200kTo500k", 0.0067448145, 0.0000045714),
        ("Q", 77, "Female", "Over500k", 0.0016317348, 0.0000020221),
        ("Q", 77, "Male", "Under50k", 0.0010227169, 0.0000380473),
        ("Q", 77, "Male", "From50kTo100k", 0.0025384642, 0.0000176595),
        ("Q", 77, "Male", "From100kTo200k", 0.0061192286, 0.0000094826),
        ("Q", 77, "Male", "From200kTo500k", 0.0096643085, 0.0000046150),
        ("Q", 77, "Male", "Over500k", 0.0040234378, 0.0000018664),
    ];

    for &(qual_str, age, gender_str, bucket_str, weight, pols_per_prem) in weights {
        let qual_status = if qual_str == "Q" { QualStatus::Q } else { QualStatus::N };
        let gender = if gender_str == "Male" { Gender::Male } else { Gender::Female };
        let benefit_base_bucket = match bucket_str {
            "Under50k" => BenefitBaseBucket::Under50k,
            "From50kTo100k" => BenefitBaseBucket::From50kTo100k,
            "From100kTo200k" => BenefitBaseBucket::From100kTo200k,
            "From200kTo500k" => BenefitBaseBucket::From200kTo500k,
            _ => BenefitBaseBucket::Over500k,
        };

        cells.push(DemographicCell {
            qual_status,
            issue_age: age,
            gender,
            benefit_base_bucket,
            base_weight: weight,
            bb_to_premium_ratio: 1.3,  // From R code: InitialPremium = InitialBB / 1.3
            pols_per_premium: pols_per_prem,  // Actual ratio from CSV
        });
    }

    cells
}

/// Build utilization rates from GLWB activation study
/// These are the incremental utilization percentages by GLWB start year
/// Extracted from pricing_inforce.csv (sum of Fixed + Indexed for each year within a cell)
fn build_utilization_rates() -> Vec<UtilizationRate> {
    // Actual utilization rates extracted from pricing_inforce.csv
    // These are incremental percentages (not cumulative) and sum to 1.0
    vec![
        UtilizationRate { glwb_start_year: 1, percentage: 0.0012735160 },
        UtilizationRate { glwb_start_year: 2, percentage: 0.0008607130 },
        UtilizationRate { glwb_start_year: 3, percentage: 0.0024944090 },
        UtilizationRate { glwb_start_year: 4, percentage: 0.0036539750 },
        UtilizationRate { glwb_start_year: 5, percentage: 0.0052282510 },
        UtilizationRate { glwb_start_year: 6, percentage: 0.0073031000 },
        UtilizationRate { glwb_start_year: 7, percentage: 0.0099526720 },
        UtilizationRate { glwb_start_year: 8, percentage: 0.0132231400 },
        UtilizationRate { glwb_start_year: 9, percentage: 0.0287011360 },
        UtilizationRate { glwb_start_year: 10, percentage: 0.0355835490 },
        UtilizationRate { glwb_start_year: 12, percentage: 0.1490581890 },
        UtilizationRate { glwb_start_year: 15, percentage: 0.2413443290 },
        UtilizationRate { glwb_start_year: 20, percentage: 0.2453262370 },
        UtilizationRate { glwb_start_year: 99, percentage: 0.2559967830 },  // Never activate
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_generation() {
        let template = InforceTemplate::new();
        let policies = template.generate(&InforceParams::default());

        // Should generate 100 cells * 14 utilizations * 2 strategies = 2800 policies
        assert_eq!(policies.len(), 2800);

        // Check total premium is approximately $100M
        let total_premium: f64 = policies.iter().map(|p| p.initial_premium).sum();
        assert!((total_premium - 100_000_000.0).abs() < 1.0);
    }

    #[test]
    fn test_fixed_only() {
        let template = InforceTemplate::new();
        let params = InforceParams {
            fixed_pct: 1.0,
            ..Default::default()
        };
        let policies = template.generate(&params);

        // All policies should be Fixed
        assert!(policies.iter().all(|p| p.crediting_strategy == CreditingStrategy::Fixed));
        // Should be 1400 policies (no Indexed)
        assert_eq!(policies.len(), 1400);
    }

    #[test]
    fn test_bonus() {
        let template = InforceTemplate::new();
        let params = InforceParams {
            bonus: 0.10,  // 10% bonus
            ..Default::default()
        };
        let policies = template.generate(&params);

        // Check that all policies have bonus set
        assert!(policies.iter().all(|p| (p.bonus - 0.10).abs() < 0.001));
    }
}
