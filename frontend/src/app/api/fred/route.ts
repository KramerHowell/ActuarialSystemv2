import { NextRequest, NextResponse } from "next/server";

const FRED_API_KEY = "14c8454ab340345288b5a2d385a86776";
const FRED_BASE_URL = "https://api.stlouisfed.org/fred/series/observations";

interface FredObservation {
  date: string;
  value: string;
}

interface FredApiResponse {
  observations: FredObservation[];
}

export async function GET(request: NextRequest): Promise<NextResponse> {
  const searchParams = request.nextUrl.searchParams;
  const seriesId = searchParams.get("series_id") || "BAMLC0A4CBBBSYTW";
  const startDate = searchParams.get("start_date") || "2020-01-01";
  const endDate = searchParams.get("end_date") || new Date().toISOString().split("T")[0];

  try {
    const url = new URL(FRED_BASE_URL);
    url.searchParams.set("series_id", seriesId);
    url.searchParams.set("api_key", FRED_API_KEY);
    url.searchParams.set("file_type", "json");
    url.searchParams.set("observation_start", startDate);
    url.searchParams.set("observation_end", endDate);
    url.searchParams.set("sort_order", "asc");

    const response = await fetch(url.toString());

    if (!response.ok) {
      throw new Error(`FRED API error: ${response.status}`);
    }

    const data: FredApiResponse = await response.json();

    // Transform data for the chart - filter out missing values
    const observations = data.observations
      .filter((obs) => obs.value !== ".")
      .map((obs) => ({
        date: obs.date,
        value: parseFloat(obs.value),
      }));

    return NextResponse.json({
      series_id: seriesId,
      observations,
      count: observations.length,
    });
  } catch (error) {
    console.error("FRED API error:", error);
    return NextResponse.json(
      { error: error instanceof Error ? error.message : "Failed to fetch FRED data" },
      { status: 500 }
    );
  }
}
