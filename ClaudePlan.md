Implementation Plan: Expenses, Commissions, and Derivatives

     Overview

     Add three features to the actuarial projection model:
     1. Expenses: 0.25%/12 of EOP AV (configurable rate)
     2. Commissions: Age-based rates with IMO/Wholesaler breakdown, conversions, chargebacks
     3. Derivatives/Hedge Gains: Model recaptured index credits from non-persisting policyholders

     ---
     1. Expenses (0.25% of EOP AV)

     Changes to src/assumptions/product.rs

     Add new field to BaseProductFeatures:
     pub expense_rate_of_av: f64,  // 0.0025 = 0.25% annual, applied monthly as /12
     - Keep annual_expense_per_policy but default to 0.0
     - New field defaults to 0.0025

     Changes to src/projection/engine.rs

     In calculate_cashflows(), change expense calculation:
     // Per COLUMN_MAPPING row AJ: =0.0025/12*AI11 (where AI = EOP AV)
     row.expenses = row.eop_av * self.assumptions.product.base.expense_rate_of_av / 12.0;
     Note: This is per-policy expense, multiply by lives if needed for total cashflow.

     ---
     2. Commissions with Age-Based Rates and Chargebacks

     New Fields in CashflowRow (src/projection/cashflows.rs)

     Replace single commission field with:
     // Commission components
     pub agent_commission: f64,           // 7% (0-75) or 4.5% (76+) of premium
     pub imo_override: f64,               // Net of conversion
     pub imo_conversion_owed: f64,        // 25% of gross IMO
     pub wholesaler_override: f64,        // Net of conversion
     pub wholesaler_conversion_owed: f64, // 40% of gross wholesaler
     pub bonus_comp: f64,                 // Month 13 bonus on BOP AV
     pub chargebacks: f64,                // Clawback of commissions for early terminations
     Remove or deprecate existing commission field.

     New Struct in src/assumptions/product.rs

     #[derive(Debug, Clone)]
     pub struct CommissionAssumptions {
         // Age thresholds
         pub age_threshold: u8,           // 75 - below uses "young" rates, at/above uses "old"

         // Agent rates
         pub agent_rate_young: f64,       // 0.07 (7%)
         pub agent_rate_old: f64,         // 0.045 (4.5%)

         // Override structure (gross rates before conversion)
         pub imo_gross_rate: f64,         // 0.036 (3.6%)
         pub wholesaler_gross_rate: f64,  // 0.006 (0.6%)
         pub override_gross_rate_old: f64, // 0.017 (1.7%) - total override for 76+

         // Conversion rates (owed back)
         pub imo_conversion_rate: f64,        // 0.25 (25%)
         pub wholesaler_conversion_rate: f64, // 0.40 (40%)

         // Bonus rates (month 13, on BOP AV)
         pub bonus_rate_young: f64,       // 0.005 (0.5%)
         // bonus_rate_old = bonus_rate_young * agent_rate_old / agent_rate_young

         // Chargeback schedule
         pub chargeback_months_full: u32,  // 6 - 100% chargeback
         pub chargeback_months_half: u32,  // 12 - 50% chargeback (months 7-12)
     }

     Commission Calculation Logic (src/projection/engine.rs)

     Month 1 only - Initial commission:
     fn calculate_initial_commission(&self, policy: &Policy, row: &mut CashflowRow) {
         let comm = &self.assumptions.product.commissions;
         let is_young = policy.issue_age <= comm.age_threshold;

         // Agent commission
         let agent_rate = if is_young { comm.agent_rate_young } else { comm.agent_rate_old };
         row.agent_commission = policy.initial_premium * agent_rate;

         // Override breakdown
         if is_young {
             // Ages 0-75: use full rates
             let imo_gross = policy.initial_premium * comm.imo_gross_rate;
             let wholesaler_gross = policy.initial_premium * comm.wholesaler_gross_rate;

             row.imo_override = imo_gross * (1.0 - comm.imo_conversion_rate);
             row.imo_conversion_owed = imo_gross * comm.imo_conversion_rate;
             row.wholesaler_override = wholesaler_gross * (1.0 - comm.wholesaler_conversion_rate);
             row.wholesaler_conversion_owed = wholesaler_gross * comm.wholesaler_conversion_rate;
         } else {
             // Ages 76+: scale by ratio
             let total_gross_young = comm.imo_gross_rate + comm.wholesaler_gross_rate; // 4.2%
             let imo_proportion = comm.imo_gross_rate / total_gross_young;
             let wholesaler_proportion = comm.wholesaler_gross_rate / total_gross_young;

             let imo_gross = policy.initial_premium * comm.override_gross_rate_old * imo_proportion;
             let wholesaler_gross = policy.initial_premium * comm.override_gross_rate_old * wholesaler_proportion;

             row.imo_override = imo_gross * (1.0 - comm.imo_conversion_rate);
             row.imo_conversion_owed = imo_gross * comm.imo_conversion_rate;
             row.wholesaler_override = wholesaler_gross * (1.0 - comm.wholesaler_conversion_rate);
             row.wholesaler_conversion_owed = wholesaler_gross * comm.wholesaler_conversion_rate;
         }
     }

     Month 13 - Bonus compensation:
     // Per COLUMN_MAPPING row AM: =IF(B11=13,O11*bonus_rate,0)
     if state.projection_month == 13 {
         let bonus_rate = if policy.issue_age <= comm.age_threshold {
             comm.bonus_rate_young
         } else {
             comm.bonus_rate_young * comm.agent_rate_old / comm.agent_rate_young
         };
         row.bonus_comp = state.bop_av * bonus_rate;
     }

     Monthly chargebacks:
     // Per COLUMN_MAPPING row AL: =AA11*(1-Z11)/$G$4*$AK$11*IF(C11>1,0,IF(B11>6,0.5,1))
     // Chargeback = lives_lost_this_month * first_month_total_commission * chargeback_factor
     fn calculate_chargebacks(&self, state: &ProjectionState, row: &mut CashflowRow, first_month_commission: f64) {
         if state.policy_year > 1 {
             row.chargebacks = 0.0;
             return;
         }

         let chargeback_factor = if state.projection_month <= 6 {
             1.0  // 100%
         } else if state.projection_month <= 12 {
             0.5  // 50%
         } else {
             0.0
         };

         // Lives lost this month = bop_lives * (1 - lives_persistency_this_month)
         let lives_lost = state.lives * (1.0 - row.lives_persistency / state.lives_persistency);

         // Chargeback applies to total commission (agent + override)
         row.chargebacks = lives_lost * first_month_commission * chargeback_factor;
     }

     State tracking needed:
     - Store month 1 total commission in ProjectionState for chargeback calculation
     - Store initial lives count for normalization

     ---
     3. Derivatives / Hedge Gains

     New Fields in ProjectionConfig (src/projection/engine.rs)

     pub struct ProjectionConfig {
         // ... existing fields ...

         /// Hedge/derivative parameters (None for Fixed products)
         pub hedge_params: Option<HedgeParams>,
     }

     #[derive(Debug, Clone)]
     pub struct HedgeParams {
         /// Option budget rate (annual) - what we pay for the derivative
         pub option_budget: f64,        // e.g., 0.0378 (same as indexed rate)

         /// Derivative appreciation rate (annual)
         pub appreciation_rate: f64,    // 0.20 (20%)

         /// Financing fee rate (annual)
         pub financing_fee: f64,        // 0.05 (5%)
     }

     Hedge Calculation Logic

     Per COLUMN_MAPPING:
     - Row AO (Net index credit reimbursement): For Indexed only, at month 12, recapture the difference between what
     we credited vs what we paid for
     - Row AP (Hedge gains): BOP_AV * (1 - AV_persistency) * option_budget * (1 + appreciation -
     financing)^(month_in_year/12) + net_reimbursement

     fn calculate_hedge_gains(&self, policy: &Policy, state: &ProjectionState, row: &mut CashflowRow) {
         // Only for Indexed products
         if policy.crediting_strategy == CreditingStrategy::Fixed {
             row.net_index_credit_reimbursement = 0.0;
             row.hedge_gains = 0.0;
             return;
         }

         let Some(params) = &self.config.hedge_params else {
             row.net_index_credit_reimbursement = 0.0;
             row.hedge_gains = 0.0;
             return;
         };

         // Rate multiplier: full rate years 1-10, half rate years 11+
         let rate_mult = if state.policy_year <= 10 { 1.0 } else { 0.5 };

         // Net appreciation factor
         let net_appreciation = 1.0 + params.appreciation_rate - params.financing_fee; // 1.15

         // Net index credit reimbursement (at annual credit time only)
         // This is: what we credited - what the derivative actually cost us
         if state.month_in_policy_year == 1 && state.policy_year > 1 {
             let indexed_rate = /* get from config */;
             let option_cost = params.option_budget * (1.0 + params.appreciation_rate);
             row.net_index_credit_reimbursement = state.bop_av * (indexed_rate - option_cost) * rate_mult;
         } else {
             row.net_index_credit_reimbursement = 0.0;
         }

         // Hedge gains from non-persisting policyholders
         // They don't get the index credit, so we pocket the appreciated derivative
         let lapsed_av = state.bop_av * (1.0 - row.av_persistency);
         row.hedge_gains = lapsed_av * params.option_budget * rate_mult
                         * net_appreciation.powf(state.month_in_policy_year as f64 / 12.0)
                         + row.net_index_credit_reimbursement;
     }

     ---
     4. Update Total Net Cashflow

     In calculate_cashflows():
     row.total_net_cashflow = row.premium
         - row.mortality_cf
         - row.lapse_cf
         - row.pwd_cf
         + row.rider_charges_cf
         + row.surrender_charges_cf
         - row.expenses
         - row.agent_commission
         - row.imo_override
         - row.wholesaler_override
         - row.bonus_comp
         + row.chargebacks  // Positive: reduces commission expense
         + row.hedge_gains;

     // Note: imo_conversion_owed and wholesaler_conversion_owed are for TRACKING ONLY
     // They don't affect total_net_cashflow - the net overrides already reflect actual payments

     ---
     Files to Modify

     ┌─────────────────────────────┬───────────────────────────────────────────────────────────────┐
     │            File             │                            Changes                            │
     ├─────────────────────────────┼───────────────────────────────────────────────────────────────┤
     │ src/projection/cashflows.rs │ Add new commission/hedge fields to CashflowRow                │
     ├─────────────────────────────┼───────────────────────────────────────────────────────────────┤
     │ src/assumptions/product.rs  │ Add expense_rate_of_av, CommissionAssumptions struct          │
     ├─────────────────────────────┼───────────────────────────────────────────────────────────────┤
     │ src/projection/engine.rs    │ Add HedgeParams, update expense/commission/hedge calculations │
     ├─────────────────────────────┼───────────────────────────────────────────────────────────────┤
     │ src/projection/state.rs     │ Add first_month_total_commission field for chargeback calc    │
     ├─────────────────────────────┼───────────────────────────────────────────────────────────────┤
     │ src/projection/mod.rs       │ Export new types                                              │
     └─────────────────────────────┴───────────────────────────────────────────────────────────────┘
     ---
     Verification

     1. Unit tests: Add tests for commission calculation at various ages
     2. Comparison: Run single policy projection and compare to Excel:
       - Check expense matches 0.0025/12 * EOP_AV
       - Check commission breakdown matches Excel columns AK, AL, AM
       - Check hedge gains match Excel column AP
     3. Run existing comparison tool: cargo run --bin compare_excel to validate no regressions