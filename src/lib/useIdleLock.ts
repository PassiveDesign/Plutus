import { useEffect, useRef } from "react";

/** Activity that counts as "the owner is still here". */
const ACTIVITY = ["pointerdown", "keydown", "wheel", "touchstart"] as const;

/** How often the idle check runs. */
const CHECK_MS = 30_000;

/**
 * Lock the vault after `idleMs` without input. Activity is tracked from real
 * DOM events, so a paused render doesn't count as activity. The check runs on
 * a timer rather than a per-event timeout so a long-open window with no input
 * still locks.
 */
export function useIdleLock(onLock: () => void, idleMs: number) {
  const last = useRef(Date.now());
  const fired = useRef(false);
  const onLockRef = useRef(onLock);
  onLockRef.current = onLock;

  useEffect(() => {
    last.current = Date.now();
    fired.current = false;

    function bump() {
      last.current = Date.now();
    }

    for (const name of ACTIVITY) window.addEventListener(name, bump, { passive: true });

    const timer = window.setInterval(() => {
      if (fired.current) return;
      if (Date.now() - last.current < idleMs) return;
      fired.current = true;
      onLockRef.current();
    }, CHECK_MS);

    return () => {
      for (const name of ACTIVITY) window.removeEventListener(name, bump);
      window.clearInterval(timer);
    };
  }, [idleMs]);
}
