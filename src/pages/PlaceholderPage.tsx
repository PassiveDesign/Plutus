import { PageDef } from "../shell/pages";

/** Stand-in for pages not built yet. Replaced page by page as milestones land.
 *  Says what the page will hold, in Passive UI's empty-state pattern. */
export function PlaceholderPage({ page }: { page: PageDef }) {
  return (
    <div className="empty placeholder">
      <span className="cap">Coming in {page.milestone}</span>
      <p className="lead">{page.summary}</p>
    </div>
  );
}
