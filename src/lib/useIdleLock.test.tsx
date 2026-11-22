import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useIdleLock } from "./useIdleLock";

const IDLE = 15 * 60 * 1000;

function Harness({ onLock, idleMs = IDLE }: { onLock: () => void; idleMs?: number }) {
  useIdleLock(onLock, idleMs);
  return null;
}

describe("useIdleLock", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("locks once the idle time passes", () => {
    const onLock = vi.fn();
    render(<Harness onLock={onLock} />);

    act(() => {
      vi.advanceTimersByTime(IDLE - 30_000);
    });
    expect(onLock).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(60_000);
    });
    expect(onLock).toHaveBeenCalledTimes(1);
  });

  it("does not lock while the owner is active", () => {
    const onLock = vi.fn();
    render(<Harness onLock={onLock} />);

    for (let i = 0; i < 40; i++) {
      act(() => {
        vi.advanceTimersByTime(60_000);
        window.dispatchEvent(new Event("pointerdown"));
      });
    }
    expect(onLock).not.toHaveBeenCalled();
  });

  it("locks only once even if the timer keeps running", () => {
    const onLock = vi.fn();
    render(<Harness onLock={onLock} />);

    act(() => {
      vi.advanceTimersByTime(IDLE * 3);
    });
    expect(onLock).toHaveBeenCalledTimes(1);
  });

  it("stops listening when unmounted", () => {
    const onLock = vi.fn();
    const { unmount } = render(<Harness onLock={onLock} />);
    unmount();

    act(() => {
      vi.advanceTimersByTime(IDLE * 2);
    });
    expect(onLock).not.toHaveBeenCalled();
  });
});
