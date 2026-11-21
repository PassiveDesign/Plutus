import { useCallback, useEffect, useMemo, useState } from "react";
import { Wallet } from "lucide-react";
import { Button } from "../components";
import { Account, api, Balances, CategoryGroup, Dashboard, errorMessage } from "../lib/api";
import { formatDate, formatMonth, thisMonth } from "../lib/dates";
import { formatCents } from "../lib/money";
import { categoryIndex } from "./CategorySelect";

type Props = {
  /** Go to another page (e.g. "accounts", "budgets", "transactions"). */
  onNavigate: (page: "accounts" | "budgets" | "transactions" | "import") => void;
  /** Open Transactions filtered to an account. */
  onOpenAccount: (accountId: number) => void;
};

function Money({ cents, big }: { cents: number; big?: boolean }) {
  return (
    <span className={["money", cents < 0 ? "negative" : null, big ? "big" : null].filter(Boolean).join(" ")}>
      {formatCents(cents)}
    </span>
  );
}

export function DashboardPage({ onNavigate, onOpenAccount }: Props) {
  const month = thisMonth();
  const [data, setData] = useState<Dashboard | null>(null);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [balances, setBalances] = useState<Balances>({});
  const [groups, setGroups] = useState<CategoryGroup[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [d, accts, bals, cats] = await Promise.all([
        api.dashboard(month),
        api.listAccounts(false),
        api.accountBalances(),
        api.listCategories(true),
      ]);
      setData(d);
      setAccounts(accts);
      setBalances(bals);
      setGroups(cats);
      setLoadError(null);
    } catch (e) {
      setLoadError(errorMessage(e));
    }
  }, [month]);

  useEffect(() => {
    void load();
  }, [load]);

  const cats = useMemo(() => categoryIndex(groups), [groups]);
  const accountName = useMemo(() => new Map(accounts.map((a) => [a.id, a.name])), [accounts]);

  if (loadError) {
    return (
      <div className="page-error" role="alert">
        <p>Couldn't load the dashboard: {loadError}</p>
        <Button variant="secondary" onClick={() => void load()}>
          Try again
        </Button>
      </div>
    );
  }
  if (data === null) return <div aria-busy="true" />;

  if (accounts.length === 0) {
    return (
      <div className="table-empty empty-cta">
        <Wallet size={36} strokeWidth={1.5} aria-hidden="true" />
        <p>Welcome to Plutus. Start by adding your bank accounts.</p>
        <Button onClick={() => onNavigate("accounts")}>Add an account</Button>
      </div>
    );
  }

  const topMax = Math.max(1, ...data.top_spending.map((t) => t.spent));
  const budgetPct =
    data.budgeted > 0 ? Math.min(100, Math.max(0, (data.budget_spent / data.budgeted) * 100)) : 0;

  return (
    <div className="dashboard">
      <div className="dash-cards">
        <section className="dash-card" aria-labelledby="nw-h">
          <h2 id="nw-h">Net worth</h2>
          <Money cents={data.net_worth.net} big />
          <p className="muted">
            Assets {formatCents(data.net_worth.assets)} · Debts {formatCents(data.net_worth.debts)}
          </p>
        </section>
        <section className="dash-card" aria-labelledby="month-h">
          <h2 id="month-h">{formatMonth(month)}</h2>
          <Money cents={data.totals.net} big />
          <p className="muted">
            In {formatCents(data.totals.income)} · Out {formatCents(data.totals.spending)}
          </p>
        </section>
        <section className="dash-card" aria-labelledby="budget-h">
          <h2 id="budget-h">Budget</h2>
          {data.budgeted > 0 ? (
            <>
              <Money cents={data.budget_remaining} big />
              <p className="muted">
                {data.budget_remaining < 0 ? "over" : "left"} of {formatCents(data.budgeted)}
              </p>
              <div
                className={`budget-bar${data.budget_remaining < 0 ? " over" : ""}`}
                role="progressbar"
                aria-label="Budget spent"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={Math.round(budgetPct)}
              >
                <span style={{ width: `${budgetPct}%` }} />
              </div>
            </>
          ) : (
            <>
              <p className="muted">No budgets set.</p>
              <Button variant="secondary" onClick={() => onNavigate("budgets")}>
                Set budgets
              </Button>
            </>
          )}
        </section>
      </div>

      <div className="dash-columns">
        <section className="dash-panel" aria-labelledby="top-h">
          <h2 id="top-h">Top spending this month</h2>
          {data.top_spending.length === 0 ? (
            <p className="muted">No spending yet this month.</p>
          ) : (
            <ul className="dash-bars">
              {data.top_spending.map((t) => {
                const name = t.category_id === null ? "Uncategorised" : (cats.get(t.category_id)?.name ?? "?");
                return (
                  <li key={t.category_id ?? "none"}>
                    <span className="dash-bar-label">{name}</span>
                    <span className="dash-bar-track" aria-hidden="true">
                      <span style={{ width: `${(t.spent / topMax) * 100}%` }} />
                    </span>
                    <Money cents={t.spent} />
                  </li>
                );
              })}
            </ul>
          )}
        </section>

        <section className="dash-panel" aria-labelledby="acct-h">
          <h2 id="acct-h">Accounts</h2>
          <ul className="dash-list">
            {accounts.map((a) => (
              <li key={a.id}>
                <button type="button" className="link-row" onClick={() => onOpenAccount(a.id)}>
                  <span>{a.name}</span>
                  <Money cents={balances[String(a.id)] ?? 0} />
                </button>
              </li>
            ))}
          </ul>
        </section>
      </div>

      <section className="dash-panel" aria-labelledby="recent-h">
        <div className="dash-panel-head">
          <h2 id="recent-h">Recent transactions</h2>
          <span className="spacer" />
          <Button variant="ghost" onClick={() => onNavigate("transactions")}>
            View all
          </Button>
        </div>
        {data.recent.length === 0 ? (
          <p className="muted">
            No transactions yet.{" "}
            <button type="button" className="link-btn" onClick={() => onNavigate("import")}>
              Import a bank export
            </button>
          </p>
        ) : (
          <ul className="dash-list">
            {data.recent.map((t) => (
              <li key={t.id} className="dash-recent">
                <span className="muted nowrap">{formatDate(t.date)}</span>
                <span className="dash-recent-payee">{t.payee ?? t.description}</span>
                <span className="muted dash-recent-acct">{accountName.get(t.account_id) ?? ""}</span>
                <Money cents={t.amount} />
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
