import { useCallback, useEffect, useMemo, useState } from "react";
import { ArrowLeftRight, Plus } from "lucide-react";
import { Button, DataTable, SelectField, TextField, useToast } from "../components";
import type { Column } from "../components";
import { Account, api, CategoryGroup, errorMessage, LedgerRow, Transaction, TransactionQuery } from "../lib/api";
import { categoryIndex, CategorySelect } from "./CategorySelect";
import { formatDate } from "../lib/dates";
import { formatCents } from "../lib/money";
import { TransactionForm, TxVerb } from "./TransactionForm";

export const PAGE_SIZE = 100;

type Props = {
  /** Start filtered to this account (e.g. after an import). */
  initialAccountId?: number | null;
};

export function TransactionsPage({ initialAccountId = null }: Props = {}) {
  const toast = useToast();
  const [accounts, setAccounts] = useState<Account[] | null>(null);
  const [accountId, setAccountId] = useState<number | null>(initialAccountId);
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [page, setPage] = useState(0);
  const [rows, setRows] = useState<LedgerRow[] | null>(null);
  const [total, setTotal] = useState(0);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [editing, setEditing] = useState<Transaction | "new" | null>(null);
  const [categories, setCategories] = useState<CategoryGroup[]>([]);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [bulkCategory, setBulkCategory] = useState<number | null>(null);
  const [bulkBusy, setBulkBusy] = useState(false);

  const filter = useMemo<TransactionQuery>(
    () => ({ account_id: accountId, from: from || null, to: to || null }),
    [accountId, from, to],
  );

  const load = useCallback(async () => {
    try {
      const [accts, txs, count, cats] = await Promise.all([
        api.listAccounts(true),
        api.listTransactions({ ...filter, limit: PAGE_SIZE, offset: page * PAGE_SIZE }),
        api.countTransactions(filter),
        api.listCategories(true),
      ]);
      setAccounts(accts);
      setCategories(cats);
      setSelected(new Set());
      setRows(txs);
      setTotal(count);
      setLoadError(null);
    } catch (e) {
      setLoadError(errorMessage(e));
    }
  }, [filter, page]);

  useEffect(() => {
    void load();
  }, [load]);

  // Any filter change goes back to the first page.
  function changeFilter(apply: () => void) {
    apply();
    setPage(0);
  }

  const byId = useMemo(() => new Map((accounts ?? []).map((a) => [a.id, a])), [accounts]);
  const activeAccounts = (accounts ?? []).filter((a) => a.archived_at === null);
  const filtered = accountId !== null || from !== "" || to !== "";

  async function toggleCleared(t: Transaction) {
    try {
      await api.updateTransaction(t.id, {
        account_id: t.account_id,
        date: t.date,
        amount: t.amount,
        payee: t.payee,
        description: t.description,
        memo: t.memo,
        cleared: !t.cleared,
        category_id: t.category_id,
      });
      void load();
    } catch (e) {
      toast.error(`Couldn't update: ${errorMessage(e)}`);
    }
  }

  const catById = useMemo(() => categoryIndex(categories), [categories]);
  const selectable = (rows ?? []).filter((t) => t.transfer_id === null);

  function toggleSelected(id: number) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  async function applyBulkCategory() {
    if (selected.size === 0 || bulkBusy) return;
    setBulkBusy(true);
    try {
      const n = await api.setTransactionCategory([...selected], bulkCategory);
      const label = bulkCategory === null ? "Uncategorised" : (catById.get(bulkCategory)?.name ?? "category");
      toast.success(`${n} transaction${n === 1 ? "" : "s"} set to ${label}.`);
      await load();
    } catch (e) {
      toast.error(`Couldn't set the category: ${errorMessage(e)}`);
    } finally {
      setBulkBusy(false);
    }
  }

  function onDone(verb: TxVerb, what: "Transaction" | "Transfer") {
    setEditing(null);
    toast.success(`${what} ${verb}.`);
    void load();
  }

  const columns: Column<LedgerRow>[] = [
    {
      key: "select",
      header: "",
      width: "36px",
      render: (t) => (
        <input
          type="checkbox"
          aria-label={`Select ${t.payee ?? t.description ?? ""}`}
          checked={selected.has(t.id)}
          disabled={t.transfer_id !== null}
          title={t.transfer_id !== null ? "Transfers don't have a category" : undefined}
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => e.stopPropagation()}
          onChange={() => toggleSelected(t.id)}
        />
      ),
    },
    { key: "date", header: "Date", width: "108px", render: (t) => <span className="nowrap">{formatDate(t.date)}</span> },
    {
      key: "payee",
      header: "Payee",
      render: (t) => {
        const title = t.payee ?? t.description ?? "";
        const sub = [t.payee ? t.description : null, t.memo].filter(Boolean).join(" · ");
        return (
          <div className="cell-stack">
            <span className="cell-title" title={title}>
              {title}
            </span>
            {sub && (
              <span className="cell-sub" title={sub}>
                {sub}
              </span>
            )}
          </div>
        );
      },
    },
    {
      key: "category",
      header: "Category",
      width: "150px",
      render: (t) => {
        if (t.transfer_id !== null) return <span className="muted">Transfer</span>;
        const c = t.category_id === null ? null : catById.get(t.category_id);
        return c ? (
          <span title={`${c.group} › ${c.name}`}>{c.name}</span>
        ) : (
          <span className="muted">—</span>
        );
      },
    },
    ...(accountId === null
      ? [
          {
            key: "account",
            header: "Account",
            width: "140px",
            render: (t: Transaction) => byId.get(t.account_id)?.name ?? "—",
          },
        ]
      : []),
    { key: "amount", header: "Amount", width: "112px", money: (t) => t.amount },
    ...(accountId !== null
      ? [
          {
            key: "balance",
            header: "Balance",
            width: "124px",
            align: "right" as const,
            render: (t: LedgerRow) =>
              t.running_balance === null ? (
                <span className="muted" title="Before the account's opening date">
                  —
                </span>
              ) : (
                <span className={`money${t.running_balance < 0 ? " negative" : ""}`}>
                  {formatCents(t.running_balance)}
                </span>
              ),
          },
        ]
      : []),
    {
      key: "cleared",
      header: "Cleared",
      width: "76px",
      align: "right",
      render: (t) => (
        <input
          type="checkbox"
          aria-label={`Cleared: ${t.payee ?? t.description ?? ""}`}
          checked={t.cleared}
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => e.stopPropagation()}
          onChange={() => void toggleCleared(t)}
        />
      ),
    },
  ];

  if (loadError) {
    return (
      <div className="page-error" role="alert">
        <p>Couldn't load transactions: {loadError}</p>
        <Button variant="secondary" onClick={() => void load()}>
          Try again
        </Button>
      </div>
    );
  }

  if (accounts === null || rows === null) return <div aria-busy="true" />;

  const first = total === 0 ? 0 : page * PAGE_SIZE + 1;
  const last = Math.min(total, (page + 1) * PAGE_SIZE);

  return (
    <div className="transactions">
      <div className="toolbar filters">
        <SelectField
          label="Account"
          value={accountId === null ? "" : String(accountId)}
          onChange={(e) => {
            const v = e.currentTarget.value;
            changeFilter(() => setAccountId(v === "" ? null : Number(v)));
          }}
        >
          <option value="">All accounts</option>
          {accounts.map((a) => (
            <option key={a.id} value={a.id}>
              {a.name}
              {a.archived_at ? " (archived)" : ""}
            </option>
          ))}
        </SelectField>
        <TextField label="From" type="date" value={from} onChange={(e) => {
          const v = e.currentTarget.value;
          changeFilter(() => setFrom(v));
        }} />
        <TextField label="To" type="date" value={to} onChange={(e) => {
          const v = e.currentTarget.value;
          changeFilter(() => setTo(v));
        }} />
        <span className="spacer" />
        <Button onClick={() => setEditing("new")} disabled={activeAccounts.length === 0}>
          <Plus size={16} aria-hidden="true" /> Add transaction
        </Button>
      </div>

      {selected.size > 0 && (
        <div className="bulk-bar" role="region" aria-label="Selected transactions">
          <span className="bulk-count">{selected.size} selected</span>
          {selected.size < selectable.length && (
            <Button variant="ghost" onClick={() => setSelected(new Set(selectable.map((t) => t.id)))}>
              Select all {selectable.length} on this page
            </Button>
          )}
          <CategorySelect
            label="Set category"
            groups={categories}
            value={bulkCategory}
            onChange={setBulkCategory}
            disabled={bulkBusy}
          />
          <Button onClick={() => void applyBulkCategory()} busy={bulkBusy} busyLabel="Applying…">
            Apply
          </Button>
          <Button variant="secondary" onClick={() => setSelected(new Set())} disabled={bulkBusy}>
            Clear selection
          </Button>
        </div>
      )}

      {activeAccounts.length === 0 && total === 0 ? (
        <div className="table-empty empty-cta">
          <ArrowLeftRight size={36} strokeWidth={1.5} aria-hidden="true" />
          <p>Add an account first, then record its transactions here.</p>
        </div>
      ) : (
        <DataTable
          caption="Transactions"
          columns={columns}
          rows={rows}
          rowKey={(t) => t.id}
          onRowClick={(t) => setEditing(t)}
          empty={filtered ? "No transactions match these filters." : "No transactions yet."}
        />
      )}

      {total > 0 && (
        <div className="pager">
          <span className="muted">
            {first}–{last} of {total}
          </span>
          <Button variant="secondary" disabled={page === 0} onClick={() => setPage((p) => p - 1)}>
            Previous
          </Button>
          <Button variant="secondary" disabled={last >= total} onClick={() => setPage((p) => p + 1)}>
            Next
          </Button>
        </div>
      )}

      {editing !== null && (
        <TransactionForm
          transaction={editing === "new" ? null : editing}
          accounts={accounts}
          categories={categories}
          defaultAccountId={accountId}
          onClose={() => setEditing(null)}
          onDone={onDone}
        />
      )}
    </div>
  );
}
