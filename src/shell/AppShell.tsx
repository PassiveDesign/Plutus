import { useEffect, useState } from "react";
import { AccountsPage } from "../pages/AccountsPage";
import { ImportPage } from "../pages/ImportPage";
import { PlaceholderPage } from "../pages/PlaceholderPage";
import { SettingsPage } from "../pages/SettingsPage";
import { TransactionsPage } from "../pages/TransactionsPage";
import { pageById, PageId } from "./pages";
import { Sidebar } from "./Sidebar";

type Props = { version: string; minPasswordChars: number; onLock: () => void };

/** Main window once the vault is unlocked: sidebar + current page. */
export function AppShell({ version, minPasswordChars, onLock }: Props) {
  const [current, setCurrent] = useState<PageId>("dashboard");
  // Account to pre-filter Transactions with when arriving from Import.
  const [txAccount, setTxAccount] = useState<number | null>(null);

  function navigate(id: PageId) {
    setTxAccount(null);
    setCurrent(id);
  }
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
      <Sidebar current={current} onNavigate={navigate} onLock={onLock} version={version} />
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
          ) : current === "transactions" ? (
            <TransactionsPage key={txAccount ?? "all"} initialAccountId={txAccount} />
          ) : current === "import" ? (
            <ImportPage
              onViewTransactions={(id) => {
                setTxAccount(id);
                setCurrent("transactions");
              }}
            />
          ) : (
            <PlaceholderPage page={page} />
          )}
        </section>
      </main>
    </div>
  );
}
