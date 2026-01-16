//! Run projection for entire block from pricing_inforce.csv
//!
//! Outputs monthly aggregated cashflows for comparison with Excel

use actuarial_system::{
    Assumptions,
    projection::{
        ProjectionEngine, ProjectionConfig, CashflowRow, CreditingApproach, HedgeParams,
        DEFAULT_FIXED_ANNUAL_RATE, DEFAULT_INDEXED_ANNUAL_RATE,
    },
};
use actuarial_system::policy::load_default_inforce;
use rayon::prelude::*;
use std::fs::File;
use std::io::Write;
use std::time::Instant;

/// Aggregated monthly results across all policies
#[derive(Debug, Clone, Default)]
struct AggregatedRow {
    month: u32,
    total_bop_av: f64,
    total_bop_bb: f64,
    total_lives: f64,
    total_mortality: f64,
    total_lapse: f64,
    total_pwd: f64,
    total_rider_charges: f64,
    total_surrender_charges: f64,
    total_interest: f64,
    total_eop_av: f64,
    // New fields
    total_expenses: f64,
    total_agent_commission: f64,
    total_imo_override: f64,
    total_wholesaler_override: f64,
    total_bonus_comp: f64,
    total_chargebacks: f64,
    total_hedge_gains: f64,
    total_net_cashflow: f64,
}

fn main() {
    env_logger::init();

    let start = Instant::now();
    println!("Loading policies from pricing_inforce.csv...");

    let policies = load_default_inforce().expect("Failed to load policies");
    println!("Loaded {} policies in {:?}", policies.len(), start.elapsed());

    // Load assumptions
    let assumptions = Assumptions::default_pricing();

    // Standard projection config - uses policy's crediting strategy
    let config = ProjectionConfig {
        projection_months: 360,
        crediting: CreditingApproach::PolicyBased {
            fixed_annual_rate: DEFAULT_FIXED_ANNUAL_RATE,
            indexed_annual_rate: DEFAULT_INDEXED_ANNUAL_RATE,
        },
        detailed_output: false, // Don't need detailed lapse components
        treasury_change: 0.0,
        fixed_lapse_rate: None,
        hedge_params: Some(HedgeParams::default()),
    };

    println!("Running projections...");
    let proj_start = Instant::now();

    // Debug: trace policy 2 (indexed) for first 14 months
    if let Some(policy) = policies.iter().find(|p| p.policy_id == 2) {
        println!("\n=== Debug: Policy 2 (Indexed) ===");
        println!("Issue age: {}, Premium: {:.2}, Crediting: {:?}",
                 policy.issue_age, policy.initial_premium, policy.crediting_strategy);
        let engine = ProjectionEngine::new(assumptions.clone(), config.clone());
        let result = engine.project_policy(policy);
        println!("Month | BOP_AV | Mort | Lapse | PWD | RiderRate | AV_Persist | AV_Lost | HedgeGains");
        for row in result.cashflows.iter().take(14) {
            // Recalculate components for debug
            let rider_rate = if row.bop_av > 0.0 {
                row.rider_charge_rate * row.bop_benefit_base / row.bop_av
            } else { 0.0 };
            let av_persist = (1.0 - row.final_mortality)
                * (1.0 - row.final_lapse_rate)
                * (1.0 - row.non_systematic_pwd_rate)
                * (1.0 - rider_rate).max(0.0);
            let av_lost = row.bop_av * (1.0 - av_persist);
            println!("{:5} | {:10.4} | {:.6} | {:.6} | {:.6} | {:.6} | {:.6} | {:.6} | {:.6}",
                     row.projection_month, row.bop_av, row.final_mortality,
                     row.final_lapse_rate, row.non_systematic_pwd_rate,
                     rider_rate, av_persist, av_lost, row.hedge_gains);
        }
        println!("=================================\n");
    }

    // Run projections in parallel
    let results: Vec<Vec<CashflowRow>> = policies
        .par_iter()
        .map(|policy| {
            let engine = ProjectionEngine::new(assumptions.clone(), config.clone());
            let result = engine.project_policy(policy);
            result.cashflows
        })
        .collect();

    println!("Projections complete in {:?}", proj_start.elapsed());

    // Aggregate results by month
    println!("Aggregating results...");
    let mut aggregated: Vec<AggregatedRow> = (1..=360)
        .map(|m| AggregatedRow { month: m, ..Default::default() })
        .collect();

    for cashflows in &results {
        for row in cashflows {
            let idx = (row.projection_month - 1) as usize;
            if idx < aggregated.len() {
                let agg = &mut aggregated[idx];
                agg.total_bop_av += row.bop_av;
                agg.total_bop_bb += row.bop_benefit_base;
                agg.total_lives += row.lives;
                agg.total_mortality += row.mortality_dec;
                agg.total_lapse += row.lapse_dec;
                agg.total_pwd += row.pwd_dec;
                agg.total_rider_charges += row.rider_charges_dec;
                agg.total_surrender_charges += row.surrender_charges_dec;
                agg.total_interest += row.interest_credits_dec;
                agg.total_eop_av += row.eop_av;
                // New fields
                agg.total_expenses += row.expenses;
                agg.total_agent_commission += row.agent_commission;
                agg.total_imo_override += row.imo_override;
                agg.total_wholesaler_override += row.wholesaler_override;
                agg.total_bonus_comp += row.bonus_comp;
                agg.total_chargebacks += row.chargebacks;
                agg.total_hedge_gains += row.hedge_gains;
                agg.total_net_cashflow += row.total_net_cashflow;
            }
        }
    }

    // Write output
    let output_path = "block_projection_output.csv";
    let mut file = File::create(output_path).expect("Failed to create output file");

    writeln!(file, "Month,BOP_AV,BOP_BB,Lives,Mortality,Lapse,PWD,RiderCharges,SurrCharges,Interest,EOP_AV,Expenses,AgentComm,IMOOverride,WholesalerOverride,BonusComp,Chargebacks,HedgeGains,NetCashflow").unwrap();

    for row in &aggregated {
        writeln!(
            file,
            "{},{:.2},{:.2},{:.8},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2}",
            row.month,
            row.total_bop_av,
            row.total_bop_bb,
            row.total_lives,
            row.total_mortality,
            row.total_lapse,
            row.total_pwd,
            row.total_rider_charges,
            row.total_surrender_charges,
            row.total_interest,
            row.total_eop_av,
            row.total_expenses,
            row.total_agent_commission,
            row.total_imo_override,
            row.total_wholesaler_override,
            row.total_bonus_comp,
            row.total_chargebacks,
            row.total_hedge_gains,
            row.total_net_cashflow,
        ).unwrap();
    }

    println!("Output written to {}", output_path);

    // Print summary stats
    println!("\nBlock Summary:");
    println!("  Month 1:   Lives={:.4}, BOP_AV=${:.0}, BOP_BB=${:.0}",
             aggregated[0].total_lives,
             aggregated[0].total_bop_av,
             aggregated[0].total_bop_bb);
    println!("  Month 60:  Lives={:.4}, BOP_AV=${:.0}",
             aggregated[59].total_lives,
             aggregated[59].total_bop_av);
    println!("  Month 120: Lives={:.4}, BOP_AV=${:.0}",
             aggregated[119].total_lives,
             aggregated[119].total_bop_av);
    println!("  Month 360: Lives={:.4}, BOP_AV=${:.0}",
             aggregated[359].total_lives,
             aggregated[359].total_bop_av);

    println!("\nTotal time: {:?}", start.elapsed());
}
