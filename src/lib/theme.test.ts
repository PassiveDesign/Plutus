import { describe, expect, it, vi } from "vitest";
import { followSystemTheme, systemMode } from "./theme";

function fakeWindow(light: boolean) {
  let listener: (() => void) | null = null;
  const query = {
    matches: light,
    addEventListener: vi.fn((_: string, fn: () => void) => (listener = fn)),
    removeEventListener: vi.fn(() => (listener = null)),
  };
  const win = { matchMedia: vi.fn(() => query) } as unknown as Window;
  return { win, query, change: (to: boolean) => ((query.matches = to), listener?.()) };
}

describe("theme", () => {
  it("reads the system preference", () => {
    expect(systemMode(fakeWindow(true).win)).toBe("light");
    expect(systemMode(fakeWindow(false).win)).toBe("dark");
  });

  it("sets data-mode now and follows changes until stopped", () => {
    const root = document.createElement("html");
    const f = fakeWindow(false);
    const stop = followSystemTheme(root, f.win);
    expect(root.dataset.mode).toBe("dark");
    f.change(true);
    expect(root.dataset.mode).toBe("light");
    stop();
    expect(f.query.removeEventListener).toHaveBeenCalled();
  });
});
