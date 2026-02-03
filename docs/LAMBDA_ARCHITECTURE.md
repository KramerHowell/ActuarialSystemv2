# Lambda API Architecture & Data Flow

This document describes the architecture of the Trellis Cost of Funds Lambda API and provides a foundation for extending it to support scenario analysis.

## System Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              FRONTEND (Vercel)                               │
│  frontend/src/app/page.tsx  ──►  frontend/src/app/api/projection/route.ts   │
└─────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼ HTTP POST (JSON)
┌─────────────────────────────────────────────────────────────────────────────┐
│                         LAMBDA (AWS Lambda + Rust)                          │
│                       src/bin/lambda_handler.rs                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                        │
                    ┌───────────────────┼───────────────────┐
                    ▼                   ▼                   ▼
            ┌──────────────┐   ┌──────────────┐   ┌──────────────┐
            │   POLICIES   │   │ ASSUMPTIONS  │   │   CONFIG     │
            └──────────────┘   └──────────────┘   └──────────────┘
                    │                   │                   │
                    └───────────────────┼───────────────────┘
                                        ▼
                            ┌──────────────────┐
                            │ PROJECTION ENGINE │
                            └──────────────────┘
                                        │
                                        ▼
                            ┌──────────────────┐
                            │    CASHFLOWS     │
                            └──────────────────┘
```

## File-by-File Data Flow

### 1. Entry Point: Lambda Handler

**File:** `src/bin/lambda_handler.rs`

```
Request JSON
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  ProjectionRequest (parsed from JSON)                           │
│  ├── projection_months: 768                                     │
│  ├── fixed_annual_rate: 0.0275                                  │
│  ├── indexed_annual_rate: 0.0378                                │
│  ├── option_budget: 0.0315                                      │
│  ├── equity_kicker: 0.20                                        │
│  ├── use_dynamic_inforce: true/false                            │
│  ├── inforce_bb_bonus: 0.34  ◄── BB bonus for rollup formula    │
│  ├── rollup_rate: 0.10                                          │
│  └── [policy filters...]                                        │
└─────────────────────────────────────────────────────────────────┘
    │
    ├──► Load Policies (one of two paths)
    │       │
    │       ├── Path A: load_adjusted_inforce() ── if use_dynamic_inforce=true
    │       │              └── src/policy/adjuster.rs
    │       │
    │       └── Path B: load_default_inforce() ── if use_dynamic_inforce=false
    │                      └── src/policy/loader.rs
    │
    ├──► Build Assumptions
    │       │
    │       └── Assumptions::default_pricing()
    │           └── src/assumptions/mod.rs
    │           │
    │           ├── assumptions.product.glwb.rollup_rate = request.rollup_rate
    │           └── assumptions.product.glwb.bonus_rate = request.inforce_bb_bonus  ◄── THE FIX
    │
    └──► Build ProjectionConfig
            └── src/projection/config.rs
```

### 2. Policy Loading

**Files:**
- `src/policy/mod.rs` - Module exports
- `src/policy/data.rs` - Policy struct definition
- `src/policy/loader.rs` - CSV loading from `data/pricing_inforce.csv`
- `src/policy/adjuster.rs` - Dynamic inforce adjustment

```
┌─────────────────────────────────────────────────────────────────┐
│  Policy Struct (src/policy/data.rs)                             │
│  ├── policy_id: u32                                             │
│  ├── qual_status: QualStatus (Q/N)                              │
│  ├── issue_age: u8                                              │
│  ├── gender: Gender (Male/Female)                               │
│  ├── initial_benefit_base: f64                                  │
│  ├── initial_pols: f64                                          │
│  ├── initial_premium: f64                                       │
│  ├── crediting_strategy: CreditingStrategy (Fixed/Indexed)      │
│  ├── sc_period: u8 (surrender charge period)                    │
│  ├── bonus: f64 (BB bonus rate for this policy)                 │
│  ├── rollup_type: RollupType (Simple/Compound)                  │
│  └── glwb_start_year: u32                                       │
└─────────────────────────────────────────────────────────────────┘
```

**Adjuster Flow (when use_dynamic_inforce=true):**
```
Base CSV (30% bonus baked in)
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  AdjustmentParams                                               │
│  ├── fixed_pct: 0.25 (Fixed vs Indexed allocation)              │
│  ├── male_mult: 1.0                                             │
│  ├── female_mult: 1.0                                           │
│  ├── qual_mult: 1.0                                             │
│  ├── nonqual_mult: 1.0                                          │
│  ├── bb_bonus: 0.34 ◄── Target BB bonus                         │
│  └── target_premium: 100,000,000                                │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
bb_bonus_factor = (1 + 0.34) / 1.3 = 1.0307...
    │
    ▼
policy.initial_benefit_base *= bb_bonus_factor
policy.bonus = 0.34
```

### 3. Assumptions

**Files:**
- `src/assumptions/mod.rs` - Assumptions struct, `default_pricing()`
- `src/assumptions/mortality.rs` - IAM 2012 mortality tables + improvement
- `src/assumptions/lapse.rs` - Predictive lapse model
- `src/assumptions/pwd.rs` - Partial withdrawal assumptions
- `src/assumptions/product.rs` - Product features (GLWB, surrender charges)

```
┌─────────────────────────────────────────────────────────────────┐
│  Assumptions Struct (src/assumptions/mod.rs)                    │
│  ├── mortality: MortalityAssumptions                            │
│  │      └── IAM 2012 + improvement rates                        │
│  ├── lapse: LapseAssumptions                                    │
│  │      └── Base rates, ITM sensitivity, shock year skew        │
│  ├── pwd: PwdAssumptions                                        │
│  │      └── Free withdrawal %, RMD rates                        │
│  └── product: ProductFeatures                                   │
│         ├── base: BaseProductFeatures                           │
│         │      └── Surrender charges, expenses                  │
│         ├── glwb: GlwbFeatures  ◄── KEY FOR ROLLUP              │
│         │      ├── bonus_rate: 0.34 ◄── Used in rollup formula  │
│         │      ├── rollup_rate: 0.10                            │
│         │      ├── rollup_years: 10                             │
│         │      └── payout_factors: by age                       │
│         └── commissions: CommissionAssumptions                  │
└─────────────────────────────────────────────────────────────────┘
```

### 4. Projection Engine

**Files:**
- `src/projection/mod.rs` - Module exports
- `src/projection/engine.rs` - Core projection logic
- `src/projection/config.rs` - ProjectionConfig
- `src/projection/cashflows.rs` - CashflowRow struct
- `src/projection/state.rs` - ProjectionState (per-policy state)

```
┌─────────────────────────────────────────────────────────────────┐
│  ProjectionEngine::new(assumptions, config)                     │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  engine.project_policy(&policy) - for each policy               │
│  │                                                              │
│  │  Month-by-Month Loop:                                        │
│  │  ├── calculate_decrements()                                  │
│  │  │      ├── Mortality rate (IAM 2012 + improvement)          │
│  │  │      ├── Lapse rate (predictive model with ITM)           │
│  │  │      ├── PWD rate                                         │
│  │  │      └── Rider charge rate                                │
│  │  │                                                           │
│  │  ├── apply_decrements()                                      │
│  │  │      └── Update AV, lives, calculate cashflows            │
│  │  │                                                           │
│  │  └── update_benefit_base() ◄── ROLLUP CALCULATION            │
│  │         │                                                    │
│  │         │  At month 12 of each policy year:                  │
│  │         │  bb_bonus = self.assumptions.product.glwb.bonus_rate│
│  │         │  rollup_factor = (1 + bb_bonus + 0.1×PY)           │
│  │         │                / (1 + bb_bonus + 0.1×(PY-1))       │
│  │         │  bop_benefit_base *= rollup_factor                 │
│  │         │                                                    │
│  └─────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  ProjectionResult                                               │
│  ├── cashflows: Vec<CashflowRow>  (768 months)                  │
│  └── reserve_result: Option<ReserveResult>                      │
└─────────────────────────────────────────────────────────────────┘
```

### 5. Aggregation & Response

```
┌─────────────────────────────────────────────────────────────────┐
│  Parallel Projection (rayon)                                    │
│  policies.par_iter().map(|p| engine.project_policy(p))          │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  Aggregation Loop                                               │
│  For each policy result:                                        │
│    For each month:                                              │
│      aggregate.bop_av += row.bop_av                             │
│      aggregate.mortality += row.mortality_dec                   │
│      aggregate.net_cashflow += row.total_net_cashflow           │
│      ... etc                                                    │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  calculate_cost_of_funds(&aggregated_cashflows)                 │
│  └── IRR calculation via Newton-Raphson                         │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  ProjectionResponse (JSON)                                      │
│  ├── cost_of_funds_pct: 3.45                                    │
│  ├── ceding_commission: { npv, rates }                          │
│  ├── policy_count: 2800                                         │
│  ├── summary: { totals }                                        │
│  └── cashflows: [ { month, bop_av, ... } × 768 ]                │
└─────────────────────────────────────────────────────────────────┘
```

## Key Files Summary

| File | Purpose | Scenario Analysis Impact |
|------|---------|-------------------------|
| `src/bin/lambda_handler.rs` | API entry point, request parsing | Add scenario parameters here |
| `src/policy/adjuster.rs` | Dynamic inforce generation | Modify for different policy mixes |
| `src/assumptions/mod.rs` | Assumption container | Create scenario-specific assumptions |
| `src/assumptions/mortality.rs` | Mortality rates | Stress test mortality |
| `src/assumptions/lapse.rs` | Lapse model | Stress test lapse behavior |
| `src/assumptions/product.rs` | GLWB features, charges | Test different product designs |
| `src/projection/engine.rs` | Core projection logic | Stable - rarely needs changes |
| `src/projection/config.rs` | Projection configuration | Add scenario-specific configs |

## Extending for Scenario Analysis

### Option 1: Multiple Assumption Sets

Create scenario-specific assumption builders:

```rust
// src/assumptions/scenarios.rs (new file)
impl Assumptions {
    pub fn base_case() -> Self { ... }
    pub fn stress_mortality(multiplier: f64) -> Self { ... }
    pub fn stress_lapse(shock: f64) -> Self { ... }
    pub fn low_interest_environment() -> Self { ... }
}
```

### Option 2: Scenario Configuration in Request

Extend the API request:

```rust
// In lambda_handler.rs
pub struct ProjectionRequest {
    // ... existing fields ...

    // Scenario overrides
    pub mortality_multiplier: Option<f64>,
    pub lapse_multiplier: Option<f64>,
    pub credited_rate_shock: Option<f64>,
    pub scenario_name: Option<String>,
}
```

### Option 3: Batch Scenario Runner

Create a new binary for running multiple scenarios:

```rust
// src/bin/scenario_runner.rs (new file)
struct ScenarioDefinition {
    name: String,
    mortality_mult: f64,
    lapse_mult: f64,
    interest_shock: f64,
}

fn main() {
    let scenarios = vec![
        ScenarioDefinition { name: "Base", ... },
        ScenarioDefinition { name: "Stress Mortality +20%", ... },
        ScenarioDefinition { name: "Mass Lapse", ... },
    ];

    for scenario in scenarios {
        let assumptions = build_assumptions(&scenario);
        let results = run_projection(assumptions);
        save_results(&scenario.name, results);
    }
}
```

### Option 4: Stochastic Scenarios (VM-22 Style)

For full VM-22 compliance, you'd need:

```
┌─────────────────────────────────────────────────────────────────┐
│  Economic Scenario Generator                                    │
│  ├── Interest rate paths (1000+ scenarios)                      │
│  ├── Equity return paths                                        │
│  └── Correlation structures                                     │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  Per-Scenario Projection                                        │
│  ├── Scenario-dependent credited rates                          │
│  ├── Scenario-dependent lapse (ITM varies by equity path)       │
│  └── Asset-side integration                                     │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  Reserve = CTE(70) of scenario results                          │
└─────────────────────────────────────────────────────────────────┘
```

## Recommended Next Steps for Scenario Analysis

1. **Start Simple**: Add mortality/lapse multipliers to the existing API
2. **Add Scenario Names**: Track which scenario produced which results
3. **Build Comparison UI**: Frontend to compare base vs stressed scenarios
4. **Batch Runner**: CLI tool to run and export multiple scenarios
5. **Stochastic (later)**: Full economic scenario generator for VM-22

## Data Files

| File | Description |
|------|-------------|
| `data/pricing_inforce.csv` | Base inforce file (2800 policies) |
| `data/assumptions/mortality_*.csv` | Mortality tables |
| `data/assumptions/surrender_charges.csv` | SC schedule |
| `data/assumptions/payout_factors.csv` | GLWB payout rates |
