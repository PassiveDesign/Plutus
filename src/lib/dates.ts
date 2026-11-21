// Calendar dates as 'YYYY-MM-DD' strings (what Rust's NaiveDate sends).
// Never go through `Date` parsing for display: that applies a timezone and
// can shift the day.

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/** Today in the user's local timezone, as YYYY-MM-DD. */
export function todayIso(now: Date = new Date()): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

/** "2026-11-05" -> "5 Nov 2026" (Australian order). Unknown input is returned as-is. */
export function formatDate(iso: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!m) return iso;
  const month = MONTHS[Number(m[2]) - 1];
  return month ? `${Number(m[3])} ${month} ${m[1]}` : iso;
}

const MONTH_NAMES = [
  "January", "February", "March", "April", "May", "June",
  "July", "August", "September", "October", "November", "December",
];

/** This month in the local timezone, as YYYY-MM. */
export function thisMonth(now: Date = new Date()): string {
  return todayIso(now).slice(0, 7);
}

/** "2026-11" -> "November 2026"; `short` gives "Nov 2026". Unknown input as-is. */
export function formatMonth(ym: string, short = false): string {
  const m = /^(\d{4})-(\d{2})$/.exec(ym);
  const i = m ? Number(m[2]) - 1 : -1;
  if (!m || i < 0 || i > 11) return ym;
  return `${short ? MONTHS[i] : MONTH_NAMES[i]} ${m[1]}`;
}

/** Step a YYYY-MM month by `delta` months. */
export function addMonths(ym: string, delta: number): string {
  const [y, m] = ym.split("-").map(Number);
  const total = y * 12 + (m - 1) + delta;
  const yy = Math.floor(total / 12);
  return `${String(yy).padStart(4, "0")}-${String((total % 12) + 1).padStart(2, "0")}`;
}
