// Passive UI 0.1.0. Built from tokens/ and src/; do not edit.
// Types for the Passive UI timeline (dist/passive-ui.mjs).

export type Window = { a: number; b: number; now: number };
export type GridTime = [ms: number, label: string];
export type SpanKind = "done" | "quiet" | "warn" | "fail";
export type BarKind = "done" | "in" | "warn" | "fail" | "next";

export type Span = { start: number; end: number; kind: SpanKind; title?: string; text?: string };
export type Bar = { at: number; size?: number; kind?: BarKind; title?: string; text?: string };
export type ExtraDue = { at: number; title?: string; text?: string };

export type LaneData = {
  spans?: Span[];
  running?: number | null;
  runningTitle?: string;
  due?: number[];
  dueTitle?: string;
  extraDue?: ExtraDue[];
  bars?: Bar[];
  watching?: boolean;
};

export type Timeline = {
  HOUR: number;
  DAY: number;
  clock(ms: number): string;
  dayName(ms: number, now?: number): string;
  when(ms: number, now?: number): string;
  span(ms: number): string;
  windowAround(opts: { back: number; ahead: number; wholeDays?: boolean }, now?: number): Window;
  pct(t: number, w: Window): number;
  gridTimes(w: Window, opts?: { hours?: number; days?: number }): GridTime[];
  axis(w: Window, times: GridTime[], opts?: { nowLabel?: string }): HTMLElement;
  lane(data: LaneData, w: Window, times: GridTime[], opts?: { size?: "row" | "big" | "bars"; bandOver?: number }): HTMLElement;
  barSizes(values: number[], opts?: { floor?: number }): number[];
  tooltips(box: HTMLElement): void;
};

export declare function createTimeline(doc: Document | null): Timeline;
export declare const timeline: Timeline;
export declare const version: string;
