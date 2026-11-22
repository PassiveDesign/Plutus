import { act, render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { shortcutLabel, usePageShortcuts } from "./usePageShortcuts";

function Harness({ onNavigate, count = 8 }: { onNavigate: (index: number) => void; count?: number }) {
  usePageShortcuts(onNavigate, count);
  return null;
}

function press(key: string, mods: { ctrlKey?: boolean; metaKey?: boolean; altKey?: boolean } = {}) {
  const e = new KeyboardEvent("keydown", { key, cancelable: true, ...mods });
  act(() => {
    window.dispatchEvent(e);
  });
  return e;
}

describe("usePageShortcuts", () => {
  it("navigates to the page at the digit's position", () => {
    const onNavigate = vi.fn();
    render(<Harness onNavigate={onNavigate} />);

    press("1", { ctrlKey: true });
    press("8", { ctrlKey: true });

    expect(onNavigate.mock.calls).toEqual([[0], [7]]);
  });

  it("accepts Cmd as well as Ctrl", () => {
    const onNavigate = vi.fn();
    render(<Harness onNavigate={onNavigate} />);

    press("3", { metaKey: true });

    expect(onNavigate).toHaveBeenCalledWith(2);
  });

  it("ignores digits without a modifier", () => {
    const onNavigate = vi.fn();
    render(<Harness onNavigate={onNavigate} />);

    press("2");

    expect(onNavigate).not.toHaveBeenCalled();
  });

  it("ignores digits beyond the page count", () => {
    const onNavigate = vi.fn();
    render(<Harness onNavigate={onNavigate} count={7} />);

    press("8", { ctrlKey: true });

    expect(onNavigate).not.toHaveBeenCalled();
  });

  it("ignores Alt+digit so system shortcuts still work", () => {
    const onNavigate = vi.fn();
    render(<Harness onNavigate={onNavigate} />);

    press("1", { ctrlKey: true, altKey: true });

    expect(onNavigate).not.toHaveBeenCalled();
  });

  it("ignores non-digit keys", () => {
    const onNavigate = vi.fn();
    render(<Harness onNavigate={onNavigate} />);

    press("l", { ctrlKey: true });
    press("Tab", { ctrlKey: true });

    expect(onNavigate).not.toHaveBeenCalled();
  });

  it("swallows the key press so the webview doesn't act on it", () => {
    const onNavigate = vi.fn();
    render(<Harness onNavigate={onNavigate} />);

    expect(press("4", { ctrlKey: true }).defaultPrevented).toBe(true);
    expect(press("4").defaultPrevented).toBe(false);
  });

  it("stops listening when unmounted", () => {
    const onNavigate = vi.fn();
    const { unmount } = render(<Harness onNavigate={onNavigate} />);
    unmount();

    press("2", { ctrlKey: true });

    expect(onNavigate).not.toHaveBeenCalled();
  });

  it("labels shortcuts one-based", () => {
    expect(shortcutLabel(0)).toBe("Ctrl+1");
    expect(shortcutLabel(7)).toBe("Ctrl+8");
  });
});
