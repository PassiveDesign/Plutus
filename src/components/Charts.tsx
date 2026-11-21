// Small hand-rolled SVG charts (no chart library: keeps the installer
// small and the look consistent). Each chart also renders its numbers as a
// visually hidden table for screen readers.

import { formatCents } from "../lib/money";

const W = 640;
const H = 220;
const PAD = { top: 12, right: 12, bottom: 28, left: 64 };

/** A "nice" upper bound for an axis (1, 2 or 5 × 10^n). */
export function niceMax(v: number): number {
  if (v <= 0) return 1;
  const p = 10 ** Math.floor(Math.log10(v));
  for (const m of [1, 2, 5, 10]) if (m * p >= v) return m * p;
  return 10 * p;
}

/** Whole-dollar axis label from cents, e.g. 250000 -> "$2.5k". */
export function axisLabel(cents: number): string {
  const d = cents / 100;
  const abs = Math.abs(d);
  const sign = d < 0 ? "-" : "";
  if (abs >= 1_000_000) return `${sign}$${+(abs / 1_000_000).toFixed(1)}m`;
  if (abs >= 1_000) return `${sign}$${+(abs / 1_000).toFixed(1)}k`;
  return `${sign}$${Math.round(abs)}`;
}

type Series = { label: string; className: string };

/** Two bars per group (e.g. income vs spending per month). */
export function PairedBarChart({
  title,
  groups,
  series,
}: {
  title: string;
  groups: { label: string; values: [number, number] }[];
  series: [Series, Series];
}) {
  const max = niceMax(Math.max(0, ...groups.flatMap((g) => g.values)));
  const innerW = W - PAD.left - PAD.right;
  const innerH = H - PAD.top - PAD.bottom;
  const slot = innerW / Math.max(1, groups.length);
  const barW = Math.max(2, Math.min(28, (slot - 6) / 2));
  const y = (v: number) => PAD.top + innerH - (Math.max(0, v) / max) * innerH;
  const ticks = [0, 0.5, 1].map((f) => f * max);
  const every = Math.ceil(groups.length / 12);
  return (
    <figure className="chart">
      <figcaption className="chart-legend">
        {series.map((s) => (
          <span key={s.label}>
            <span className={`swatch ${s.className}`} aria-hidden="true" /> {s.label}
          </span>
        ))}
      </figcaption>
      <svg viewBox={`0 0 ${W} ${H}`} role="img" aria-label={title}>
        {ticks.map((t) => (
          <g key={t}>
            <line className="chart-grid" x1={PAD.left} x2={W - PAD.right} y1={y(t)} y2={y(t)} />
            <text className="chart-axis" x={PAD.left - 8} y={y(t) + 4} textAnchor="end">
              {axisLabel(t)}
            </text>
          </g>
        ))}
        {groups.map((g, i) => {
          const x0 = PAD.left + i * slot + (slot - barW * 2 - 2) / 2;
          return (
            <g key={g.label}>
              {g.values.map((v, j) => (
                <rect
                  key={j}
                  className={series[j].className}
                  x={x0 + j * (barW + 2)}
                  y={y(v)}
                  width={barW}
                  height={PAD.top + innerH - y(v)}
                >
                  <title>{`${g.label} ${series[j].label}: ${formatCents(v)}`}</title>
                </rect>
              ))}
              {i % every === 0 && (
                <text className="chart-axis" x={PAD.left + i * slot + slot / 2} y={H - 8} textAnchor="middle">
                  {g.label}
                </text>
              )}
            </g>
          );
        })}
      </svg>
      <table className="sr-only">
        <caption>{title}</caption>
        <thead>
          <tr>
            <th scope="col">Month</th>
            <th scope="col">{series[0].label}</th>
            <th scope="col">{series[1].label}</th>
          </tr>
        </thead>
        <tbody>
          {groups.map((g) => (
            <tr key={g.label}>
              <th scope="row">{g.label}</th>
              <td>{formatCents(g.values[0])}</td>
              <td>{formatCents(g.values[1])}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </figure>
  );
}

/** Axis bounds that fit the data (not forced to include zero, so a large
 *  but slowly changing figure like net worth with a mortgage still shows
 *  its trend). Returns [low, high, ticks]. */
export function lineBounds(values: number[]): [number, number, number[]] {
  if (values.length === 0) return [0, 1, [0, 1]];
  let lo = Math.min(...values);
  let hi = Math.max(...values);
  const span = hi - lo || Math.max(100_00, Math.abs(hi) * 0.05);
  lo -= span * 0.15;
  hi += span * 0.15;
  const step = niceMax((hi - lo) / 3);
  lo = Math.floor(lo / step) * step;
  hi = Math.ceil(hi / step) * step;
  const ticks: number[] = [];
  for (let t = lo; t <= hi + step / 2; t += step) ticks.push(t);
  return [lo, hi, ticks];
}

/** A single line with dots (e.g. net worth at each month end). */
export function LineChart({ title, points }: { title: string; points: { label: string; value: number }[] }) {
  const [bottom, top, ticks] = lineBounds(points.map((p) => p.value));
  const span = top - bottom || 1;
  const inset = 24; // keep first/last dots and labels inside the frame
  const innerW = W - PAD.left - PAD.right - inset * 2;
  const innerH = H - PAD.top - PAD.bottom;
  const x = (i: number) =>
    PAD.left + inset + (points.length <= 1 ? innerW / 2 : (i / (points.length - 1)) * innerW);
  const y = (v: number) => PAD.top + ((top - v) / span) * innerH;
  const every = Math.ceil(points.length / 12);
  const path = points.map((p, i) => `${i === 0 ? "M" : "L"}${x(i).toFixed(1)},${y(p.value).toFixed(1)}`).join(" ");
  return (
    <figure className="chart">
      <svg viewBox={`0 0 ${W} ${H}`} role="img" aria-label={title}>
        {ticks.map((t) => (
          <g key={t}>
            <line className={t === 0 ? "chart-zero" : "chart-grid"} x1={PAD.left} x2={W - PAD.right} y1={y(t)} y2={y(t)} />
            <text className="chart-axis" x={PAD.left - 8} y={y(t) + 4} textAnchor="end">
              {axisLabel(t)}
            </text>
          </g>
        ))}
        <path className="chart-line" d={path} />
        {points.map((p, i) => (
          <g key={p.label}>
            <circle className={`chart-dot${p.value < 0 ? " negative" : ""}`} cx={x(i)} cy={y(p.value)} r={3.5}>
              <title>{`${p.label}: ${formatCents(p.value)}`}</title>
            </circle>
            {i % every === 0 && (
              <text className="chart-axis" x={x(i)} y={H - 8} textAnchor="middle">
                {p.label}
              </text>
            )}
          </g>
        ))}
      </svg>
      <table className="sr-only">
        <caption>{title}</caption>
        <tbody>
          {points.map((p) => (
            <tr key={p.label}>
              <th scope="row">{p.label}</th>
              <td>{formatCents(p.value)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </figure>
  );
}
