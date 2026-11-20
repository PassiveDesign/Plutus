import { useEffect, useState } from "react";
import { PlaceholderPage } from "../pages/PlaceholderPage";
import { pageById, PageId } from "./pages";
import { Sidebar } from "./Sidebar";

type Props = { version: string; onLock: () => void };

/** Main window once the vault is unlocked: sidebar + current page. */
export function AppShell({ version, onLock }: Props) {
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
      <Sidebar current={current} onNavigate={setCurrent} onLock={onLock} version={version} />
      <main className="content">
        <header className="page-header">
          <h1>{page.label}</h1>
        </header>
        <section className="page-body">
          {/* Real pages replace the placeholder here as roadmap tasks land. */}
          <PlaceholderPage page={page} />
        </section>
      </main>
    </div>
  );
}
