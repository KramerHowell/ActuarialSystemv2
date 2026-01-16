import { NextRequest, NextResponse } from "next/server";
import { spawn } from "child_process";
import path from "path";

interface ProjectionRequest {
  projection_months?: number;
  fixed_annual_rate?: number;
  indexed_annual_rate?: number;
  treasury_change?: number;
  bbb_rate?: number;  // BBB rate for ceding commission (as decimal, e.g., 0.05 for 5%)
  spread?: number;    // Spread for ceding commission (as decimal)
  // Dynamic inforce parameters
  use_dynamic_inforce?: boolean;
  inforce_fixed_pct?: number;
  inforce_male_mult?: number;
  inforce_female_mult?: number;
  inforce_qual_mult?: number;
  inforce_nonqual_mult?: number;
  inforce_bb_bonus?: number;
  rollup_rate?: number;
}

interface ProjectionSummary {
  total_premium: number;
  total_initial_av: number;
  total_initial_bb: number;
  total_initial_lives: number;
  total_net_cashflows: number;
  month_1_cashflow: number;
  final_lives: number;
  final_av: number;
}

interface CedingCommission {
  npv: number;
  bbb_rate_pct: number;
  spread_pct: number;
  total_rate_pct: number;
}

interface InforceParamsOutput {
  fixed_pct: number;
  male_mult: number;
  female_mult: number;
  qual_mult: number;
  nonqual_mult: number;
  bonus: number;
}

interface DetailedCashflowRow {
  month: number;
  bop_av: number;
  bop_bb: number;
  lives: number;
  mortality: number;
  lapse: number;
  pwd: number;
  rider_charges: number;
  surrender_charges: number;
  interest: number;
  eop_av: number;
  expenses: number;
  agent_commission: number;
  imo_override: number;
  wholesaler_override: number;
  bonus_comp: number;
  chargebacks: number;
  hedge_gains: number;
  net_cashflow: number;
}

interface ProjectionResponse {
  cost_of_funds_pct: number | null;
  ceding_commission?: CedingCommission | null;
  inforce_params?: InforceParamsOutput | null;
  policy_count: number;
  projection_months: number;
  summary: ProjectionSummary;
  cashflows: DetailedCashflowRow[];
  execution_time_ms: number;
  error?: string;
}

// Default values matching the Rust code
const DEFAULT_PROJECTION_MONTHS = 768;
const DEFAULT_FIXED_ANNUAL_RATE = 0.0275;
const DEFAULT_INDEXED_ANNUAL_RATE = 0.0378;

export async function POST(request: NextRequest): Promise<NextResponse> {
  const start = Date.now();

  try {
    const body: ProjectionRequest = await request.json();

    const projectionMonths = body.projection_months ?? DEFAULT_PROJECTION_MONTHS;
    const fixedAnnualRate = body.fixed_annual_rate ?? DEFAULT_FIXED_ANNUAL_RATE;
    const indexedAnnualRate = body.indexed_annual_rate ?? DEFAULT_INDEXED_ANNUAL_RATE;
    const treasuryChange = body.treasury_change ?? 0;
    const bbbRate = body.bbb_rate;  // Optional - only calculate ceding commission if provided
    const spread = body.spread ?? 0;

    // Dynamic inforce parameters
    const useDynamicInforce = body.use_dynamic_inforce ?? false;
    const inforceFixedPct = body.inforce_fixed_pct ?? 0.25;
    const inforceMaleMult = body.inforce_male_mult ?? 1.0;
    const inforceFemaleMult = body.inforce_female_mult ?? 1.0;
    const inforceQualMult = body.inforce_qual_mult ?? 1.0;
    const inforceNonqualMult = body.inforce_nonqual_mult ?? 1.0;
    const inforceBBBonus = body.inforce_bb_bonus ?? 0.30;
    const rollupRate = body.rollup_rate ?? 0.10;

    // For development: run the Rust binary directly
    // In production, this would call AWS Lambda
    const projectRoot = path.resolve(process.cwd(), "..");
    const binaryPath = path.join(projectRoot, "target", "release", "cost_of_funds");

    // Check if we should use Lambda or local binary
    const useLambda = process.env.USE_LAMBDA === "true";

    if (useLambda && process.env.LAMBDA_FUNCTION_URL) {
      // Call Lambda function
      const lambdaResponse = await fetch(process.env.LAMBDA_FUNCTION_URL, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          projection_months: projectionMonths,
          fixed_annual_rate: fixedAnnualRate,
          indexed_annual_rate: indexedAnnualRate,
          treasury_change: treasuryChange,
        }),
      });

      if (!lambdaResponse.ok) {
        throw new Error(`Lambda error: ${lambdaResponse.status}`);
      }

      const data = await lambdaResponse.json();
      return NextResponse.json(data);
    }

    // Local development: run Rust binary and parse output
    const result = await runLocalProjection(
      binaryPath,
      projectRoot,
      projectionMonths,
      fixedAnnualRate,
      indexedAnnualRate,
      treasuryChange,
      bbbRate,
      spread,
      useDynamicInforce,
      inforceFixedPct,
      inforceMaleMult,
      inforceFemaleMult,
      inforceQualMult,
      inforceNonqualMult,
      inforceBBBonus,
      rollupRate
    );

    return NextResponse.json({
      ...result,
      execution_time_ms: Date.now() - start,
    });
  } catch (error) {
    console.error("Projection error:", error);
    return NextResponse.json(
      {
        cost_of_funds_pct: null,
        policy_count: 0,
        projection_months: DEFAULT_PROJECTION_MONTHS,
        summary: {
          total_premium: 0,
          total_initial_av: 0,
          total_initial_bb: 0,
          total_initial_lives: 0,
          total_net_cashflows: 0,
          month_1_cashflow: 0,
          final_lives: 0,
          final_av: 0,
        },
        execution_time_ms: Date.now() - start,
        error: error instanceof Error ? error.message : "Unknown error",
      } as ProjectionResponse,
      { status: 500 }
    );
  }
}

async function runLocalProjection(
  binaryPath: string,
  projectRoot: string,
  projectionMonths: number,
  fixedAnnualRate: number,
  indexedAnnualRate: number,
  treasuryChange: number,
  bbbRate?: number,
  spread?: number,
  useDynamicInforce?: boolean,
  inforceFixedPct?: number,
  inforceMaleMult?: number,
  inforceFemaleMult?: number,
  inforceQualMult?: number,
  inforceNonqualMult?: number,
  inforceBBBonus?: number,
  rollupRate?: number
): Promise<ProjectionResponse> {
  return new Promise((resolve) => {
    // Set environment variables for the projection config
    const env: NodeJS.ProcessEnv = {
      ...process.env,
      PROJECTION_MONTHS: projectionMonths.toString(),
      FIXED_ANNUAL_RATE: fixedAnnualRate.toString(),
      INDEXED_ANNUAL_RATE: indexedAnnualRate.toString(),
      TREASURY_CHANGE: treasuryChange.toString(),
    };

    // Add BBB rate and spread if provided for ceding commission calculation
    if (bbbRate !== undefined) {
      env.BBB_RATE = bbbRate.toString();
    }
    if (spread !== undefined) {
      env.SPREAD = spread.toString();
    }

    // Add dynamic inforce parameters
    if (useDynamicInforce) {
      env.USE_DYNAMIC_INFORCE = "1";
      if (inforceFixedPct !== undefined) {
        env.INFORCE_FIXED_PCT = inforceFixedPct.toString();
      }
      if (inforceMaleMult !== undefined) {
        env.INFORCE_MALE_MULT = inforceMaleMult.toString();
      }
      if (inforceFemaleMult !== undefined) {
        env.INFORCE_FEMALE_MULT = inforceFemaleMult.toString();
      }
      if (inforceQualMult !== undefined) {
        env.INFORCE_QUAL_MULT = inforceQualMult.toString();
      }
      if (inforceNonqualMult !== undefined) {
        env.INFORCE_NONQUAL_MULT = inforceNonqualMult.toString();
      }
      if (inforceBBBonus !== undefined) {
        env.INFORCE_BB_BONUS = inforceBBBonus.toString();
      }
      if (rollupRate !== undefined) {
        env.ROLLUP_RATE = rollupRate.toString();
      }
    }

    // Use --json flag for structured output
    const child = spawn(binaryPath, ["--json"], {
      cwd: projectRoot,
      env,
    });

    let stdout = "";
    let stderr = "";

    child.stdout.on("data", (data) => {
      stdout += data.toString();
    });

    child.stderr.on("data", (data) => {
      stderr += data.toString();
    });

    child.on("close", (code) => {
      if (code !== 0) {
        // If binary failed, return mock data for development
        console.error("Binary failed:", stderr);
        resolve(getMockResponse(projectionMonths));
        return;
      }

      // Parse JSON output from the Rust binary
      try {
        const result = JSON.parse(stdout.trim());
        resolve(result);
      } catch {
        console.error("Failed to parse JSON output:", stdout);
        resolve(getMockResponse(projectionMonths));
      }
    });

    child.on("error", (err) => {
      console.error("Failed to spawn binary:", err);
      // Return mock data if binary doesn't exist
      resolve(getMockResponse(projectionMonths));
    });
  });
}

function getMockResponse(projectionMonths: number): ProjectionResponse {
  // Mock response for development when binary isn't available
  return {
    cost_of_funds_pct: 5.24,
    policy_count: 806,
    projection_months: projectionMonths,
    summary: {
      total_premium: 100000000,
      total_initial_av: 100000000,
      total_initial_bb: 130000000,
      total_initial_lives: 806.57,
      total_net_cashflows: -45000000,
      month_1_cashflow: -98000000,
      final_lives: 0.0001,
      final_av: 0,
    },
    cashflows: Array(projectionMonths).fill(0).map((_, i) => ({
      month: i + 1,
      bop_av: 100000000 * Math.exp(-i / 200),
      bop_bb: 130000000 * Math.exp(-i / 300),
      lives: 806 * Math.exp(-i / 200),
      mortality: 1000,
      lapse: 500,
      pwd: 200,
      rider_charges: 0,
      surrender_charges: 0,
      interest: 50000,
      eop_av: 100000000 * Math.exp(-i / 200),
      expenses: 20000,
      agent_commission: i === 0 ? 6000000 : 0,
      imo_override: i === 0 ? 2500000 : 0,
      wholesaler_override: i === 0 ? 340000 : 0,
      bonus_comp: 0,
      chargebacks: 4000,
      hedge_gains: 1000,
      net_cashflow: i === 0 ? 90000000 : -100000 * Math.exp(-i / 100),
    })),
    execution_time_ms: 0,
  };
}
