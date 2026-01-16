

we have three final things to include in our actuarial model before we apply a front end for a "cost of funds" calculator: 1) can we add an expense (through an input) of 0.25%/12 of the EOP AV, 2) we need to add commissions--these vary by age and are shown in commissions_20260115.xlsx. we also have
  "chargebacks" of commissions for full terminations in the first year (100% in the first 6 months, 50% in the next 6 months, 0% after year 1). you can see these most easily in COLUMN_MAPPING.md. 3) we also need to model derivatives (in a simplified manner for now). because policyholders who don't persist
  to the end of the period don't receive an index credit, we get to pocket the derivative (which we assume is appreciating at 20% per year) less a financing fee (assume this is 5% per year). This is also dem



Summary of Requirements

  1. Expenses (0.25%/12 of EOP AV)
  - Change from fixed per-policy expense to 0.25%/12 of EOP AV
  - Per COLUMN_MAPPING.md (row AJ): =0.0025/12*AI11 (where AI = EOP AV)

  2. Commissions with Chargebacks
  From the commissions file:
  ┌──────────┬───────┬──────────┬────────────────┐
  │ Age Band │ Agent │ Override │ Month 13 Bonus │
  ├──────────┼───────┼──────────┼────────────────┤
  │ 0-75     │ 7%    │ 3.06%    │ 0.5%           │
  ├──────────┼───────┼──────────┼────────────────┤
  │ 76+      │ 4.5%  │ 1.2386%  │ 0.3214%        │
  └──────────┴───────┴──────────┴────────────────┘
  - Chargebacks: 100% of commission clawed back if full termination in months 1-6, 50% in months 7-12, 0% after year 1
  - Bonus comp paid at month 13 based on BOP AV

  3. Derivatives/Hedge Gains (Indexed products only)
  - Policyholders who lapse/die don't receive the index credit, so we recapture:
    - The derivative appreciating at 20%/year
    - Less financing fee of 5%/year
  - Formula: BOP_AV * (1-AV_persistency) * option_budget * (1 + 0.20 - 0.05)^(month/12)

  Questions

  1. For the expense input of 0.25% - should this be hardcoded or configurable via an input parameter? Currently the code has a fixed per-policy expense in BaseProductFeatures.
  2. For commissions - do you want the Agent + Override combined (total of ~10% for 0-75, ~5.7% for 76+), or tracked separately?
  3. For derivatives - the Excel references $X$4 (option budget), $Y$4 (20% appreciation), $Z$4 (indexed rate), $AA$4 (5% financing). Should these be configurable or hardcoded for now?

  1) let's have it be configurable and remove the per-policy expense (or leave it as an input, but have it be $0 for now). 2) let's track this separately, and there's a bit more complexity to this because the overrides have some reductions we'll want to track eventually. the 3.06% is actually 3.6% * (1-25%
  IMO equity conversion) + 0.6% * (1-40% wholesaler conversion). then the 1.238...% is 1.7% * (1-conversion factor), which is essentially 1.7%*3.06%/4.2%. we'll want to be able to track the amount of conversion owed to the IMOs and wholesalers separately. the age 76+ bonus amount is the 0.5% 0-75 bonus *
  the upfront street ratio (4.5%/7%). 3) let's have all of these be inputs 

  Got it - this is more nuanced than I initially understood. Let me make sure I have the commission structure right:

  Commission Breakdown:
  ┌───────────────────────┬───────────┬───────────────────────────┬──────────────────────┐
  │       Component       │ Ages 0-75 │         Ages 76+          │        Notes         │
  ├───────────────────────┼───────────┼───────────────────────────┼──────────────────────┤
  │ Agent (street)        │ 7%        │ 4.5%                      │ Direct to agent      │
  ├───────────────────────┼───────────┼───────────────────────────┼──────────────────────┤
  │ IMO gross             │ 3.6%      │ 1.4%                      │ Before conversion    │
  ├───────────────────────┼───────────┼───────────────────────────┼──────────────────────┤
  │ IMO conversion        │ 25%       │ 25%                       │ Equity owed back     │
  ├───────────────────────┼───────────┼───────────────────────────┼──────────────────────┤
  │ Wholesaler gross      │ 0.6%      │ 0.3%                      │ Before conversion    │
  ├───────────────────────┼───────────┼───────────────────────────┼──────────────────────┤
  │ Wholesaler conversion │ 40%       │ 40%                       │ Owed back            │
  ├───────────────────────┼───────────┼───────────────────────────┼──────────────────────┤
  │ Net override          │ 3.06%     │ 1.2386%                   │ What we actually pay │
  ├───────────────────────┼───────────┼───────────────────────────┼──────────────────────┤
  │ Month 13 bonus        │ 0.5%      │ 0.5% * (street 76+ / street 0-75) ≈ 0.32%    │ On BOP AV            │
  └───────────────────────┴───────────┴───────────────────────────┴──────────────────────┘
  Tracking separately:
  - Agent commission paid
  - IMO override paid (net of conversion)
  - IMO conversion owed
  - Wholesaler override paid (net of conversion)
  - Wholesaler conversion owed
  - Bonus comp
  - Chargebacks (reduce total)

  A few clarifying questions:

  1. For the 76+ override calculation, is the scaling factor applied to both IMO and Wholesaler portions? (i.e., both scaled by 4.5%/7% ratio, or is only IMO scaled by the 3.06%/4.2% ratio you mentioned?)
  2. For chargebacks, per the Excel formula =AA11*(1-Z11)/$G$4*$AK$11*IF(C11>1,0,IF(B11>6,0.5,1)) - this applies to the first-month commission ($AK$11) proportional to lives lost. Should chargebacks apply to Agent only, or to Agent + Override?
  3. Should I add these as new fields to the CashflowRow struct (agent_commission, imo_override, imo_conversion_owed, wholesaler_override, wholesaler_conversion_owed, bonus_comp, chargebacks), or do you want a separate commission detail structure?

❯ 1) here is the override calculation: =1.7%*(3.6*0.75+0.6*0.6)/4.2. 2) it applies to all commissions (agent + override). 3) yes let's add them to the CashflowRow struct