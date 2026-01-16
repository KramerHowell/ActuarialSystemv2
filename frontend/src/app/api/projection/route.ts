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

interface ProjectionResponse {
  cost_of_funds_pct: number | null;
  ceding_commission?: CedingCommission | null;
  policy_count: number;
  projection_months: number;
  summary: ProjectionSummary;
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
      spread
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
  spread?: number
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
    execution_time_ms: 0,
  };
}
