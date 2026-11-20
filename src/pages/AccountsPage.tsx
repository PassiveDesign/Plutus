import { useCallback, useEffect, useMemo, useState } from "react";
import { Plus, Wallet } from "lucide-react";
import { Button, DataTable, useToast } from "../components";
import type { Column } from "../components";
import { formatBsb, groupAccounts, KIND_LABELS, totalCents } from "../lib/accounts";
import { Account, api, errorMessage } from "../lib/api";
import { formatCents } from "../lib/money";
import { AccountForm, SavedVerb } from "./AccountForm";

export function AccountsPage() {
  const toast = useToast();
  // All accounts (archived included) so offset links can always be named;
  // the archive filter is applied here.
  const [all, setAll] = useState<Account[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [showArchived, setShowArchived] = useState(false);
  const [editing, setEditing] = useState<Account | "new" | null>(null);

  const load = useCallback(async () => {
    try {
      setAll(await api.listAccounts(true));
      setLoadError(null);
    } catch (e) {
      setLoadError(errorMessage(e));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const byId = useMemo(() => new Map((all ?? []).map((a) => [a.id, a])), [all]);
  const active = useMemo(() => (all ?? []).filter((a) => a.archived_at === null), [all]);
  const visible = showArchived ? (all ?? []) : active;
  const archivedCount = (all?.length ?? 0) - active.length;

  function onSaved(account: Account, verb: SavedVerb) {
    setEditing(null);
    toast.success(`${account.name} ${verb}.`);
    void load();
  }

  const subtitle = (a: Account): string | null => {
    const parts: string[] = [];
    if (a.institution) parts.push(a.institution);
    if (a.offset_for !== null) {
      parts.push(`Offsets ${byId.get(a.offset_for)?.name ?? "a loan"}`);
    }
    return parts.length ? parts.join(" · ") : null;
  };

  const columns: Column<Account>[] = [
    {
      key: "name",
      header: "Account",
      render: (a) => (
        <div className="cell-stack">
          <span className="cell-title">
            {a.name}
            {a.archived_at && <span className="badge">Archived</span>}
          </span>
          {subtitle(a) && <span className="cell-sub">{subtitle(a)}</span>}
        </div>
      ),
    },
    { key: "kind", header: "Type", width: "130px", render: (a) => KIND_LABELS[a.kind] },
    {
      key: "numbers",
      header: "BSB / Account",
      width: "190px",
      render: (a) => (
        <span className="mono">
          {[formatBsb(a.bsb), a.account_number].filter(Boolean).join(" · ")}
        </span>
      ),
    },
    { key: "balance", header: "Balance", width: "160px", money: (a) => a.opening_balance },
  ];

  if (loadError) {
    return (
      <div className="page-error" role="alert">
        <p>Couldn't load accounts: {loadError}</p>
        <Button variant="secondary" onClick={() => void load()}>
          Try again
        </Button>
      </div>
    );
  }

  if (all === null) return <div aria-busy="true" />;

  return (
    <div className="accounts">
      <div className="toolbar">
        <div className="net-total">
          <span className="muted">Net total</span>
          <strong className={totalCents(active) < 0 ? "negative" : undefined}>
            {formatCents(totalCents(active))}
          </strong>
        </div>
        <span className="spacer" />
        {archivedCount > 0 && (
          <label className="check">
            <input
              type="checkbox"
              checked={showArchived}
              onChange={(e) => setShowArchived(e.currentTarget.checked)}
            />
            Show archived ({archivedCount})
          </label>
        )}
        <Button onClick={() => setEditing("new")}>
          <Plus size={16} aria-hidden="true" /> Add account
        </Button>
      </div>

      {visible.length === 0 ? (
        <div className="table-empty empty-cta">
          <Wallet size={36} strokeWidth={1.5} aria-hidden="true" />
          <p>No accounts yet. Add your bank accounts, offsets, cards and loans to get started.</p>
          <Button onClick={() => setEditing("new")}>Add your first account</Button>
        </div>
      ) : (
        groupAccounts(visible).map(({ group, accounts }) => (
          <section key={group.id} className="account-group" aria-labelledby={`grp-${group.id}`}>
            <h2 id={`grp-${group.id}`} className="group-heading">
              <span>{group.label}</span>
              <span className={`money${totalCents(accounts.filter((a) => !a.archived_at)) < 0 ? " negative" : ""}`}>
                {formatCents(totalCents(accounts.filter((a) => !a.archived_at)))}
              </span>
            </h2>
            <DataTable
              caption={group.label}
              columns={columns}
              rows={accounts}
              rowKey={(a) => a.id}
              onRowClick={(a) => setEditing(a)}
              rowClassName={(a) => (a.archived_at ? "archived" : undefined)}
            />
          </section>
        ))
      )}

      {editing !== null && (
        <AccountForm
          account={editing === "new" ? null : editing}
          onClose={() => setEditing(null)}
          onSaved={onSaved}
        />
      )}
    </div>
  );
}
