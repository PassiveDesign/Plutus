import { PageId, PAGES, SETTINGS_PAGE } from "./pages";

type Props = {
  current: PageId;
  onNavigate: (id: PageId) => void;
  onLock: () => void;
};

/** The bar across the top: name, one tab per page, and Lock. Passive UI's
 *  shell, so Plutus sits beside PassiveSync as one family. */
export function TopBar({ current, onNavigate, onLock }: Props) {
  const tabs = [...PAGES, SETTINGS_PAGE];
  return (
    <header className="top">
      <div className="logo" aria-label="Plutus">
        plu<span>tus</span>
      </div>
      <nav className="tabs" aria-label="Main">
        {tabs.map((p) => (
          <button
            key={p.id}
            type="button"
            aria-current={p.id === current ? "page" : undefined}
            aria-selected={p.id === current}
            onClick={() => onNavigate(p.id)}
          >
            {p.label}
          </button>
        ))}
      </nav>
      <div className="bar-actions">
        <button type="button" onClick={onLock} title="Lock (Ctrl+L)" aria-keyshortcuts="Control+L">
          Lock<kbd aria-hidden="true">Ctrl L</kbd>
        </button>
      </div>
    </header>
  );
}
