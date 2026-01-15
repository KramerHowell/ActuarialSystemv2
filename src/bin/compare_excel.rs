//! Compare Rust lapse model with Excel spreadsheet values
//! Test case: SC=10, Income@5, BB=27000 (bucket 0), ITM=1.3

use actuarial_system::assumptions::LapseModel;
use actuarial_system::policy::BenefitBaseBucket;

fn main() {
    let model = LapseModel::default_predictive_model();

    // Excel inputs: SC=10, Income starts year 5, BB=27000 (bucket 0), ITM=1.3
    let sc_period = 10u32;
    let income_start_year = 5u32;
    let bucket = BenefitBaseBucket::Under50k; // 27000 is in [0, 50000)
    let itm = 1.3f64;

    println!("Rust vs Excel comparison (SC=10, Income@5, BB=27000, ITM=1.3)");
    println!("{:<6} {:<8} {:<14} {:<14} {:<14} {:<10}",
             "Year", "Income", "Rust_LP", "Excel_LP", "Diff", "LapseRate");

    // Excel reference values (from spreadsheet)
    let excel_values = [
        (1, false, -5.35992714),
        (2, false, -4.87146763),
        (3, false, -4.38025328),
        (4, false, -3.88628407),
        (5, true,  -4.37652044),
        (6, true,  -4.28931300),
        (7, true,  -4.19800509),
        (8, true,  -4.10259673),
        (9, true,  -4.00308791),
        (10, true, -3.89947862),
        (11, true, -2.98128140),
        (12, true, -3.29557550),
        (13, true, -3.26273783),
    ];

    for (year, income_on, excel_lp) in excel_values.iter() {
        let base = model.base_component_with_bucket(
            *year as u32,
            *income_on,
            bucket,
            sc_period,
        );
        let dynamic = model.dynamic_component(itm, *income_on);
        let rust_lp = base + dynamic;
        let diff = rust_lp - excel_lp;
        let lapse_rate = rust_lp.min(0.0).exp();

        println!("{:<6} {:<8} {:<14.8} {:<14.8} {:<14.8} {:<10.8}",
                 year,
                 if *income_on { "Yes" } else { "No" },
                 rust_lp,
                 excel_lp,
                 diff,
                 lapse_rate);
    }
}
