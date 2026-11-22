import { Lock } from "lucide-react";
import { shortcutLabel } from "../lib/usePageShortcuts";
import { PageDef, PageId, PAGES, SETTINGS_PAGE } from "./pages";

type Props = {
  current: PageId;
  onNavigate: (id: PageId) => void;
  onLock: () => void;
  version: string;
};

function NavItem({
  page,
  active,
  index,
  onClick,
}: {
  page: PageDef;
  active: boolean;
  /** Position in sidebar order, for the shortcut hint. */
  index: number;
  onClick: () => void;
}) {
  const Icon = page.icon;
  return (
    <li>
      <button
        type="button"
        className={`nav-item${active ? " active" : ""}`}
        aria-current={active ? "page" : undefined}
        title={`${page.label} (${shortcutLabel(index)})`}
        onClick={onClick}
      >
        <Icon size={18} aria-hidden="true" />
        <span>{page.label}</span>
      </button>
    </li>
  );
}

export function Sidebar({ current, onNavigate, onLock, version }: Props) {
  return (
    <nav className="sidebar" aria-label="Main">
      <div className="brand">Plutus</div>

      <ul className="nav-list">
        {PAGES.map((p, i) => (
          <NavItem
            key={p.id}
            page={p}
            active={p.id === current}
            index={i}
            onClick={() => onNavigate(p.id)}
          />
        ))}
      </ul>

      <div className="sidebar-footer">
        <ul className="nav-list">
          <NavItem
            page={SETTINGS_PAGE}
            active={current === SETTINGS_PAGE.id}
            index={PAGES.length}
            onClick={() => onNavigate(SETTINGS_PAGE.id)}
          />
          <li>
            <button type="button" className="nav-item" onClick={onLock} title="Lock (Ctrl+L)">
              <Lock size={18} aria-hidden="true" />
              <span>Lock</span>
            </button>
          </li>
        </ul>
        <div className="version">v{version}</div>
      </div>
    </nav>
  );
}
