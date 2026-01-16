"use client";

import { useState, useEffect, useCallback } from "react";
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  ReferenceLine,
} from "recharts";
import { format, parseISO, subYears } from "date-fns";

interface Observation {
  date: string;
  value: number;
}

interface FredChartProps {
  onRateSelect?: (rate: number, date: string) => void;
}

const DATE_RANGES = [
  { label: "1Y", years: 1 },
  { label: "3Y", years: 3 },
  { label: "5Y", years: 5 },
  { label: "10Y", years: 10 },
  { label: "Max", years: 30 },
];

export default function FredChart({ onRateSelect }: FredChartProps) {
  const [data, setData] = useState<Observation[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedPoint, setSelectedPoint] = useState<Observation | null>(null);
  const [dateRange, setDateRange] = useState(5); // Default 5 years

  const fetchData = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    const endDate = new Date().toISOString().split("T")[0];
    const startDate = subYears(new Date(), dateRange).toISOString().split("T")[0];

    try {
      const response = await fetch(
        `/api/fred?series_id=BAMLC0A4CBBBSYTW&start_date=${startDate}&end_date=${endDate}`
      );

      if (!response.ok) {
        throw new Error("Failed to fetch data");
      }

      const result = await response.json();
      setData(result.observations);

      // Set initial selected point to most recent
      if (result.observations.length > 0) {
        const latest = result.observations[result.observations.length - 1];
        setSelectedPoint(latest);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "An error occurred");
    } finally {
      setIsLoading(false);
    }
  }, [dateRange]);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const handleChartClick = (data: any) => {
    if (data?.activePayload && data.activePayload.length > 0) {
      const point = data.activePayload[0].payload as Observation;
      setSelectedPoint(point);
      if (onRateSelect) {
        onRateSelect(point.value, point.date);
      }
    }
  };

  const formatXAxis = (dateStr: string) => {
    try {
      return format(parseISO(dateStr), "MMM yy");
    } catch {
      return dateStr;
    }
  };

  const formatTooltipDate = (dateStr: string) => {
    try {
      return format(parseISO(dateStr), "MMM d, yyyy");
    } catch {
      return dateStr;
    }
  };

  // Calculate stats
  const latestValue = data.length > 0 ? data[data.length - 1].value : null;
  const minValue = data.length > 0 ? Math.min(...data.map((d) => d.value)) : 0;
  const maxValue = data.length > 0 ? Math.max(...data.map((d) => d.value)) : 10;
  const avgValue = data.length > 0 ? data.reduce((sum, d) => sum + d.value, 0) / data.length : 0;

  // Calculate change
  const firstValue = data.length > 0 ? data[0].value : null;
  const change = latestValue && firstValue ? latestValue - firstValue : null;

  return (
    <div className="space-y-4">
      {/* Header with current rate */}
      <div className="flex justify-between items-start">
        <div>
          <h3 className="text-lg font-semibold flex items-center gap-2">
            ICE BofA BBB US Corporate Index
          </h3>
          <p className="text-sm text-[--text-muted]">BAMLC0A4CBBBSYTW - Semi-Annual Yield to Worst</p>
        </div>
        <div className="text-right">
          {selectedPoint ? (
            <>
              <p className="text-3xl font-bold text-[--accent]">
                {selectedPoint.value.toFixed(2)}%
              </p>
              <p className="text-sm text-[--text-muted]">
                {formatTooltipDate(selectedPoint.date)}
              </p>
            </>
          ) : latestValue ? (
            <>
              <p className="text-3xl font-bold text-[--accent]">{latestValue.toFixed(2)}%</p>
              <p className="text-sm text-[--text-muted]">Latest</p>
            </>
          ) : (
            <p className="text-[--text-muted]">--</p>
          )}
        </div>
      </div>

      {/* Date range selector */}
      <div className="flex gap-2">
        {DATE_RANGES.map((range) => (
          <button
            key={range.label}
            onClick={() => setDateRange(range.years)}
            className={`px-3 py-1 rounded text-sm font-medium transition-colors ${
              dateRange === range.years
                ? "bg-[--accent] text-[--bg-primary]"
                : "bg-[--bg-secondary] text-[--text-muted] hover:text-[--text-primary]"
            }`}
          >
            {range.label}
          </button>
        ))}
      </div>

      {/* Chart */}
      <div className="h-64 w-full">
        {isLoading ? (
          <div className="h-full flex items-center justify-center text-[--text-muted]">
            <span className="animate-pulse">Loading FRED data...</span>
          </div>
        ) : error ? (
          <div className="h-full flex items-center justify-center text-red-400">
            Error: {error}
          </div>
        ) : (
          <ResponsiveContainer width="100%" height="100%">
            <LineChart
              data={data}
              onClick={handleChartClick}
              margin={{ top: 5, right: 5, left: 0, bottom: 5 }}
            >
              <CartesianGrid strokeDasharray="3 3" stroke="var(--border-color)" />
              <XAxis
                dataKey="date"
                tickFormatter={formatXAxis}
                stroke="var(--text-muted)"
                tick={{ fill: "var(--text-muted)", fontSize: 12 }}
                interval="preserveStartEnd"
                minTickGap={50}
              />
              <YAxis
                domain={[Math.floor(minValue - 0.5), Math.ceil(maxValue + 0.5)]}
                stroke="var(--text-muted)"
                tick={{ fill: "var(--text-muted)", fontSize: 12 }}
                tickFormatter={(value) => `${value}%`}
                width={50}
              />
              <Tooltip
                contentStyle={{
                  backgroundColor: "var(--bg-card)",
                  border: "1px solid var(--border-color)",
                  borderRadius: "8px",
                  color: "var(--text-primary)",
                }}
                labelFormatter={formatTooltipDate}
                formatter={(value) => [`${Number(value).toFixed(2)}%`, "BBB Yield"]}
              />
              {selectedPoint && (
                <ReferenceLine
                  x={selectedPoint.date}
                  stroke="var(--accent)"
                  strokeDasharray="3 3"
                />
              )}
              <Line
                type="monotone"
                dataKey="value"
                stroke="var(--accent)"
                strokeWidth={2}
                dot={false}
                activeDot={{
                  r: 6,
                  fill: "var(--accent)",
                  stroke: "var(--bg-primary)",
                  strokeWidth: 2,
                  cursor: "pointer",
                }}
              />
            </LineChart>
          </ResponsiveContainer>
        )}
      </div>

      {/* Stats row */}
      <div className="grid grid-cols-4 gap-4 pt-2 border-t border-[--border-color]">
        <div>
          <p className="text-xs text-[--text-muted]">Current</p>
          <p className="font-semibold">{latestValue?.toFixed(2)}%</p>
        </div>
        <div>
          <p className="text-xs text-[--text-muted]">Change ({dateRange}Y)</p>
          <p className={`font-semibold ${change && change > 0 ? "text-red-400" : "text-green-400"}`}>
            {change ? `${change > 0 ? "+" : ""}${change.toFixed(2)}%` : "--"}
          </p>
        </div>
        <div>
          <p className="text-xs text-[--text-muted]">Low</p>
          <p className="font-semibold">{minValue.toFixed(2)}%</p>
        </div>
        <div>
          <p className="text-xs text-[--text-muted]">High</p>
          <p className="font-semibold">{maxValue.toFixed(2)}%</p>
        </div>
      </div>

      {/* Instructions */}
      <p className="text-xs text-[--text-muted] text-center">
        Click or tap on the chart to select a specific date
      </p>
    </div>
  );
}
