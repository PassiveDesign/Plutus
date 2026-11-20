import { useEffect, useState } from "react";
import { AccountsPage } from "../pages/AccountsPage";
import { PlaceholderPage } from "../pages/PlaceholderPage";
import { SettingsPage } from "../pages/SettingsPage";
import { pageById, PageId } from "./pages";
import { StatusBar } from "./StatusBar";
import { TopBar } from "./TopBar";

type Props = { version: string; minPasswordChars: number; onLock: () => void };

/** Main window once the vault is unlocked: top bar, current page, status bar. */
export function AppShell({ version, minPasswordChars, onLock }: Props) {
  const [current, setCurrent] = useState<PageId>("dashboard");
  const page = pageById(current);

  // Ctrl+L (Cmd+L on macOS) locks the app from anywhere.
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "l") {
        e.preventDefault();
        onLock();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onLock]);

  return (
    <div className="shell">
      <TopBar current={current} onNavigate={setCurrent} onLock={onLock} />
      <main className="content">
        <header className="page-header">
          <h1>{page.label}</h1>
        </header>
        <section className="page-body">
          {/* Real pages replace the placeholder here as roadmap tasks land. */}
          {current === "settings" ? (
            <SettingsPage minChars={minPasswordChars} />
          ) : current === "accounts" ? (
            <AccountsPage />
          ) : (
            <PlaceholderPage page={page} />
          )}
        </section>
      </main>
      <StatusBar version={version} />
    </div>
  );
}
