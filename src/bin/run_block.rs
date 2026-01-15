//! Run projection for entire block from pricing_inforce.csv
//!
//! Outputs monthly aggregated cashflows for comparison with Excel

use actuarial_system::{
    Assumptions,
    projection::{
        ProjectionEngine, ProjectionConfig, CashflowRow, CreditingApproach,
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
    };

    println!("Running projections...");
    let proj_start = Instant::now();

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
            }
        }
    }

    // Write output
    let output_path = "block_projection_output.csv";
    let mut file = File::create(output_path).expect("Failed to create output file");

    writeln!(file, "Month,BOP_AV,BOP_BB,Lives,Mortality,Lapse,PWD,RiderCharges,SurrCharges,Interest,EOP_AV").unwrap();

    for row in &aggregated {
        writeln!(
            file,
            "{},{:.2},{:.2},{:.8},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2}",
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
