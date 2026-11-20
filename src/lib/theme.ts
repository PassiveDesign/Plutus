// Light or dark, following the operating system. Passive UI reads the
// choice from <html data-mode>; this keeps it in step as the system changes.

export type Mode = "light" | "dark";

/** The system's current preference. */
export function systemMode(win: Window = window): Mode {
  return win.matchMedia?.("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

/** Sets `data-mode` on the root now and whenever the system changes.
 *  Returns a function that stops following. */
export function followSystemTheme(root: HTMLElement = document.documentElement, win: Window = window): () => void {
  const apply = () => {
    root.dataset.mode = systemMode(win);
  };
  apply();
  const query = win.matchMedia?.("(prefers-color-scheme: light)");
  query?.addEventListener?.("change", apply);
  return () => query?.removeEventListener?.("change", apply);
}
