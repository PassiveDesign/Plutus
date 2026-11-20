import { PageDef } from "../shell/pages";

/** Stand-in for pages not built yet. Replaced page by page as milestones land. */
export function PlaceholderPage({ page }: { page: PageDef }) {
  const Icon = page.icon;
  return (
    <div className="placeholder">
      <Icon size={40} strokeWidth={1.5} aria-hidden="true" />
      <p>{page.summary}</p>
      <p className="muted">Coming in {page.milestone}.</p>
    </div>
  );
}
