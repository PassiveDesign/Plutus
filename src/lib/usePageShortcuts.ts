import { useEffect } from "react";

/** Label shown in the sidebar tooltip for the page at `index` (0-based). */
export function shortcutLabel(index: number): string {
  return `Ctrl+${index + 1}`;
}

/**
 * Ctrl/Cmd + digit jumps to the page at that position in the sidebar.
 *
 * Every shortcut carries a modifier, so typing in a text field never triggers
 * one and no "is the user typing" check is needed. Alt is excluded so system
 * shortcuts (Alt+1 on some layouts) stay untouched.
 */
export function usePageShortcuts(onNavigate: (index: number) => void, count: number) {
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (!(e.ctrlKey || e.metaKey) || e.altKey) return;
      const n = Number(e.key);
      if (!Number.isInteger(n) || n < 1 || n > count) return;
      e.preventDefault();
      onNavigate(n - 1);
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onNavigate, count]);
}
