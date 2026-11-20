/* Passive UI 0.1.0. Built from tokens/ and src/; do not edit. */
// Passive UI · timeline. A lane of time for one thing: spans for what it
// did, a span for what it is doing, dashes for what it is due to do, and
// bars whose height is a quantity. No framework: it builds DOM elements,
// so a plain page appends them and React puts them in a ref.
//
// Times are milliseconds. A window is { a, b, now }: start, end, and now.

function createTimeline(doc) {
  const DAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
  const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  const HOUR = 3600000;
  const DAY = 24 * HOUR;
  const pad2 = (n) => String(n).padStart(2, "0");

  /** "14:05" */
  const clock = (ms) => {
    const d = new Date(ms);
    return `${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
  };

  /** "today", "yesterday", "tomorrow", or "Mon 18", relative to `now`. */
  function dayName(ms, now = Date.now()) {
    const d = new Date(ms);
    const t = new Date(now);
    const days = Math.round(
      (new Date(d.getFullYear(), d.getMonth(), d.getDate()) - new Date(t.getFullYear(), t.getMonth(), t.getDate())) / DAY,
    );
    if (days === 0) return "today";
    if (days === -1) return "yesterday";
    if (days === 1) return "tomorrow";
    return `${DAYS[d.getDay()]} ${d.getDate()}`;
  }
  const when = (ms, now) => `${dayName(ms, now)} ${clock(ms)}`;

  /** "12m", "3h", "2d" for a span of milliseconds. */
  function span(ms) {
    const mins = Math.max(0, Math.round(ms / 60000));
    if (mins < 60) return `${mins}m`;
    if (mins < 60 * 36) return `${Math.round(mins / 60)}h`;
    return `${Math.round(mins / 1440)}d`;
  }

  /**
   * A window of time around now. `back` and `ahead` are hours. With
   * `wholeDays`, the window is widened to midnight at both ends.
   */
  function windowAround({ back, ahead, wholeDays = false }, now = Date.now()) {
    if (!wholeDays) return { a: now - back * HOUR, b: now + ahead * HOUR, now };
    const start = new Date(now - back * HOUR);
    start.setHours(0, 0, 0, 0);
    const end = new Date(now + ahead * HOUR);
    end.setHours(24, 0, 0, 0);
    return { a: start.getTime(), b: end.getTime(), now };
  }

  /** Where a time falls across a window, as a percentage. */
  const pct = (t, w) => ((t - w.a) / (w.b - w.a)) * 100;

  /**
   * Times for grid lines and their labels: every `hours` hours on the
   * even hour, or every `days` days at midnight.
   */
  function gridTimes(w, { hours = 0, days = 0 } = {}) {
    const out = [];
    if (days) {
      const d = new Date(w.a);
      d.setHours(0, 0, 0, 0);
      if (d.getTime() < w.a) d.setDate(d.getDate() + 1);
      let i = 0;
      while (d.getTime() <= w.b) {
        if (i % days === 0) {
          out.push([d.getTime(), days === 1 ? `${DAYS[d.getDay()]} ${d.getDate()}` : `${d.getDate()} ${MONTHS[d.getMonth()]}`]);
        }
        d.setDate(d.getDate() + 1);
        i += 1;
      }
      return out;
    }
    const step = (hours || 2) * HOUR;
    const d = new Date(w.a);
    d.setMinutes(0, 0, 0);
    while (d.getTime() <= w.a || d.getHours() % (hours || 2) !== 0) d.setHours(d.getHours() + 1);
    for (let t = d.getTime(); t <= w.b; t += step) out.push([t, clock(t)]);
    return out;
  }

  function el(tag, cls, style) {
    const e = doc.createElement(tag);
    if (cls) e.className = cls;
    if (style) e.style.cssText = style;
    return e;
  }

  /** Puts what a mark was into it, for the tooltip. */
  function tip(e, title, text) {
    e.dataset.tipTitle = title ?? "";
    e.dataset.tipText = text ?? "";
    return e;
  }

  /**
   * The axis under a lane. Labels too close to "now" are left out so the
   * two do not overlap.
   */
  function axis(w, times, { nowLabel = "now" } = {}) {
    const box = el("div", "axis");
    for (const [t, text] of times) {
      if (Math.abs(pct(t, w) - pct(w.now, w)) <= 4) continue;
      const s = el("span", "", `left:${pct(t, w)}%`);
      s.textContent = text;
      box.append(s);
    }
    const n = el("span", "now-label", `left:${pct(w.now, w)}%`);
    n.textContent = nowLabel;
    box.append(n);
    return box;
  }

  /**
   * A lane.
   *   spans:   [{ start, end, kind, title, text }]  kind: done | quiet | warn | fail
   *   running: start time of what is under way, or null
   *   due:     [time]  what it is due to do; more than `bandOver` become one band
   *   bars:    [{ at, size (0..1), kind, title, text }]  kind: done | in | warn | fail | next
   *   watching: draws a dotted line from now on (it acts whenever things change)
   *   size:    "row" (fills its cell), "big" (34px), "bars" (64px, for bars)
   */
  function lane(
    { spans = [], running = null, runningTitle, due = [], dueTitle, bars = [], watching = false, extraDue = [] } = {},
    w,
    times,
    { size = "row", bandOver = 48 } = {},
  ) {
    const box = el("div", size === "row" ? "lane" : `lane ${size}`);
    for (const [t] of times) box.append(el("i", "tick", `left:${pct(t, w)}%`));
    if (watching) box.append(el("i", "watch", `left:${pct(w.now, w)}%;right:0`));
    const mark = (kind, from, to, title, text) => {
      const left = Math.max(0, pct(from, w));
      const right = Math.min(100, pct(Math.max(to, from), w));
      return tip(el("i", `b ${kind}`, `left:${left}%;width:${Math.max(0, right - left)}%`), title, text);
    };
    for (const s of spans) {
      if (s.end < w.a || s.start > w.b) continue;
      box.append(mark(s.kind, s.start, s.end, s.title, s.text));
    }
    const ahead = due.filter((t) => t > w.now && t <= w.b);
    if (ahead.length > bandOver) {
      const first = ahead[0];
      const last = ahead[ahead.length - 1];
      box.append(mark("band", first, last, `Due ${ahead.length} times`, `From ${when(first, w.now)} to ${when(last, w.now)}`));
    } else {
      for (const t of ahead) box.append(mark("next", t, t, `Due · ${when(t, w.now)}`, dueTitle ?? ""));
    }
    for (const x of extraDue) {
      if (x.at > w.now && x.at <= w.b) box.append(mark("next", x.at, x.at, x.title, x.text));
    }
    if (running) box.append(mark("run", running, w.now, runningTitle ?? `Running · since ${clock(running)}`, ""));
    for (const bar of bars) {
      if (bar.at < w.a || bar.at > w.b) continue;
      const tall = Math.max(6, Math.min(100, (bar.size ?? 0.3) * 100));
      box.append(tip(el("i", `tk ${bar.kind ?? "done"}`, `left:${pct(bar.at, w)}%;height:${tall}%`), bar.title, bar.text));
    }
    box.append(el("i", "now", `left:${pct(w.now, w)}%`));
    return box;
  }

  /**
   * Sizes for bars: the square root of each value over the largest, so one
   * big value does not flatten the rest. Returns 0..1 for each.
   */
  function barSizes(values, { floor = 0.1 } = {}) {
    const most = Math.max(1e-9, ...values.map((v) => Math.abs(v)));
    return values.map((v) => Math.max(floor, Math.sqrt(Math.abs(v) / most)));
  }

  /**
   * Shows the tooltip element `box` over any element with data-tip-title,
   * following the pointer. Call once per page.
   */
  function tooltips(box) {
    doc.addEventListener("mousemove", (event) => {
      const target = event.target.closest?.("[data-tip-title]");
      if (!target) {
        box.hidden = true;
        return;
      }
      const title = el("b");
      title.textContent = target.dataset.tipTitle;
      box.replaceChildren(title, target.dataset.tipText ?? "");
      box.hidden = false;
      const view = doc.defaultView;
      const x = Math.min(event.clientX + 14, view.innerWidth - box.offsetWidth - 8);
      const y = event.clientY + 18 + box.offsetHeight > view.innerHeight ? event.clientY - box.offsetHeight - 10 : event.clientY + 18;
      box.style.left = `${x}px`;
      box.style.top = `${y}px`;
    });
  }

  return { HOUR, DAY, clock, dayName, when, span, windowAround, pct, gridTimes, axis, lane, barSizes, tooltips };
}

export const version = "0.1.0";
export const timeline = createTimeline(typeof document === "undefined" ? null : document);
export { createTimeline };
