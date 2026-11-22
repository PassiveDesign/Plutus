import { useCallback, useEffect, useMemo, useState } from "react";
import { Button, SelectField } from "../components";
import { LineChart, PairedBarChart } from "../components/Charts";
import { api, CategoryGroup, errorMessage, Report } from "../lib/api";
import { addMonths, formatMonth, thisMonth } from "../lib/dates";
import { formatCents } from "../lib/money";
import { categoryIndex } from "./CategorySelect";

export type Range = "6m" | "12m" | "ytd" | "last-year";

/** Inclusive [from, to] months for a range, relative to `now` (YYYY-MM). */
export function rangeMonths(range: Range, now: string): [string, string] {
  const year = now.slice(0, 4);
  switch (range) {
    case "6m":
      return [addMonths(now, -5), now];
    case "12m":
      return [addMonths(now, -11), now];
    case "ytd":
      return [`${year}-01`, now];
    case "last-year": {
      const y = String(Number(year) - 1).padStart(4, "0");
      return [`${y}-01`, `${y}-12`];
    }
  }
}

const RANGE_LABELS: Record<Range, string> = {
  "6m": "Last 6 months",
  "12m": "Last 12 months",
  ytd: "This year",
  "last-year": "Last year",
};

function Money({ cents }: { cents: number }) {
  return <span className={`money${cents < 0 ? " negative" : ""}`}>{formatCents(cents)}</span>;
}

export function ReportsPage({ now = thisMonth() }: { now?: string }) {
  const [range, setRange] = useState<Range>("6m");
  const [data, setData] = useState<Report | null>(null);
  const [groups, setGroups] = useState<CategoryGroup[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [from, to] = rangeMonths(range, now);

  const load = useCallback(async () => {
    try {
      const [r, cats] = await Promise.all([api.report(from, to), api.listCategories(true)]);
      setData(r);
      setGroups(cats);
      setLoadError(null);
    } catch (e) {
      setLoadError(errorMessage(e));
    }
  }, [from, to]);

  useEffect(() => {
    void load();
  }, [load]);

  const cats = useMemo(() => categoryIndex(groups), [groups]);

  if (loadError) {
    return (
      <div className="page-error" role="alert">
        <p>Couldn't load the report: {loadError}</p>
        <Button variant="secondary" onClick={() => void load()}>
          Try again
        </Button>
      </div>
    );
  }

  const spendTotal = data?.spending_by_category.reduce((s, c) => s + c.spent, 0) ?? 0;
  const topMax = Math.max(1, ...(data?.spending_by_category.map((c) => c.spent) ?? []));
  const short = (m: string) => formatMonth(m, true).replace(/ (\d{2})(\d{2})$/, " ’$2");

  return (
    <div className="reports">
      <div className="toolbar filters">
        <SelectField label="Period" value={range} onChange={(e) => setRange(e.currentTarget.value as Range)}>
          {(Object.keys(RANGE_LABELS) as Range[]).map((r) => (
            <option key={r} value={r}>
              {RANGE_LABELS[r]}
            </option>
          ))}
        </SelectField>
        <span className="muted reports-range">
          {formatMonth(from)} – {formatMonth(to)}
        </span>
      </div>

      {data === null ? (
        <div aria-busy="true" />
      ) : (
        <>
          <div className="budget-summary">
            <div>
              <span className="muted">Income</span>
              <Money cents={data.totals.income} />
            </div>
            <div>
              <span className="muted">Spending</span>
              <Money cents={data.totals.spending} />
            </div>
            <div>
              <span className="muted">Saved</span>
              <Money cents={data.totals.net} />
            </div>
          </div>

          {data.totals.income === 0 &&
            data.totals.spending === 0 &&
            data.spending_by_category.length === 0 && (
              <div className="table-empty empty-cta">
                <p>Nothing to report yet.</p>
                <p className="muted">
                  Import transactions or add some by hand and this fills in.
                </p>
              </div>
            )}

          <section className="dash-panel" aria-labelledby="inout-h">
            <h2 id="inout-h">Income vs spending</h2>
            <PairedBarChart
              title="Income and spending by month"
              groups={data.months.map((m) => ({ label: short(m.month), values: [m.income, m.spending] }))}
              series={[
                { label: "Income", className: "bar-income" },
                { label: "Spending", className: "bar-spending" },
              ]}
            />
          </section>

          <section className="dash-panel" aria-labelledby="nw-h">
            <h2 id="nw-h">Net worth</h2>
            <LineChart
              title="Net worth at the end of each month"
              points={data.months.map((m) => ({ label: short(m.month), value: m.net_worth }))}
            />
          </section>

          <section className="dash-panel" aria-labelledby="cat-h">
            <h2 id="cat-h">Spending by category</h2>
            {data.spending_by_category.length === 0 ? (
              <p className="muted">No spending in this period.</p>
            ) : (
              <ul className="dash-bars report-bars">
                {data.spending_by_category.map((c) => {
                  const info = c.category_id === null ? null : cats.get(c.category_id);
                  const name = c.category_id === null ? "Uncategorised" : (info?.name ?? "?");
                  return (
                    <li key={c.category_id ?? "none"}>
                      <span className="dash-bar-label" title={info ? `${info.group} › ${info.name}` : undefined}>
                        {name}
                      </span>
                      <span className="dash-bar-track" aria-hidden="true">
                        <span style={{ width: `${(c.spent / topMax) * 100}%` }} />
                      </span>
                      <span className="muted report-pct">
                        {spendTotal > 0 ? Math.round((c.spent / spendTotal) * 100) : 0}%
                      </span>
                      <Money cents={c.spent} />
                    </li>
                  );
                })}
              </ul>
            )}
          </section>
        </>
      )}
    </div>
  );
}
