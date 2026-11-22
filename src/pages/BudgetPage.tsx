import { KeyboardEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, RotateCcw } from "lucide-react";
import { Button, useToast } from "../components";
import { api, BudgetLine, CategoryGroup, errorMessage, MonthBudget } from "../lib/api";
import { addMonths, formatMonth, thisMonth } from "../lib/dates";
import { formatCents, plainCents } from "../lib/money";

function Money({ cents, className }: { cents: number; className?: string }) {
  return (
    <span className={["money", cents < 0 ? "negative" : null, className].filter(Boolean).join(" ")}>
      {formatCents(cents)}
    </span>
  );
}

type Props = {
  /** Start on this month (YYYY-MM); defaults to the current month. */
  initialMonth?: string;
};

export function BudgetPage({ initialMonth }: Props) {
  const toast = useToast();
  const [month, setMonth] = useState(initialMonth ?? thisMonth());
  const [data, setData] = useState<MonthBudget | null>(null);
  const [groups, setGroups] = useState<CategoryGroup[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [editing, setEditing] = useState<number | null>(null);

  const load = useCallback(async () => {
    try {
      const [b, cats] = await Promise.all([api.budgetMonth(month), api.listCategories(true)]);
      setData(b);
      setGroups(cats);
      setLoadError(null);
    } catch (e) {
      setLoadError(errorMessage(e));
    }
  }, [month]);

  useEffect(() => {
    void load();
  }, [load]);

  const names = useMemo(() => {
    const m = new Map<number, string>();
    for (const g of groups) {
      m.set(-g.id, g.name);
      for (const c of g.categories) m.set(c.id, c.name);
    }
    return m;
  }, [groups]);

  async function act(action: () => Promise<void>, failure: string) {
    try {
      await action();
      await load();
    } catch (e) {
      toast.error(`${failure}: ${errorMessage(e)}`);
    }
  }

  if (loadError) {
    return (
      <div className="page-error" role="alert">
        <p>Couldn't load the budget: {loadError}</p>
        <Button variant="secondary" onClick={() => void load()}>
          Try again
        </Button>
      </div>
    );
  }

  // Lines grouped in tree order.
  const byGroup: { id: number; lines: BudgetLine[] }[] = [];
  for (const l of data?.lines ?? []) {
    const last = byGroup[byGroup.length - 1];
    if (last && last.id === l.group_id) last.lines.push(l);
    else byGroup.push({ id: l.group_id, lines: [l] });
  }

  return (
    <div className="budget">
      <div className="toolbar">
        <Button variant="ghost" aria-label="Previous month" onClick={() => setMonth((m) => addMonths(m, -1))}>
          <ChevronLeft size={18} aria-hidden="true" />
        </Button>
        <h2 className="budget-month" aria-live="polite">
          {formatMonth(month)}
        </h2>
        <Button variant="ghost" aria-label="Next month" onClick={() => setMonth((m) => addMonths(m, 1))}>
          <ChevronRight size={18} aria-hidden="true" />
        </Button>
        {month !== thisMonth() && (
          <Button variant="secondary" onClick={() => setMonth(thisMonth())}>
            This month
          </Button>
        )}
      </div>

      {data === null ? (
        <div aria-busy="true" />
      ) : (
        <>
          <div className="budget-summary">
            <div>
              <span className="muted">Budgeted</span>
              <Money cents={data.total_budgeted} />
            </div>
            <div>
              <span className="muted">Spent</span>
              <Money cents={data.total_spent_budgeted} />
            </div>
            <div>
              <span className="muted">{data.total_remaining < 0 ? "Over" : "Left"}</span>
              <Money cents={data.total_remaining} />
            </div>
          </div>
          <p className="muted budget-extra">
            Spending without a budget {formatCents(data.unbudgeted_spent)} · Uncategorised{" "}
            {formatCents(data.uncategorised_spent)} · Income {formatCents(data.income)}
          </p>

          {byGroup.length === 0 ? (
            <div className="table-empty empty-cta">
              <p>No categories to budget yet.</p>
              <p className="muted">
                Add categories on the Categories page, then set a monthly amount here.
              </p>
            </div>
          ) : (
          <div className="table-wrap">
            <table className="data-table budget-table">
              <caption className="sr-only">Budget for {formatMonth(month)}</caption>
              <thead>
                <tr>
                  <th scope="col">Category</th>
                  <th scope="col" className="num" style={{ width: "150px" }}>
                    Budget
                  </th>
                  <th scope="col" className="num" style={{ width: "120px" }}>
                    Spent
                  </th>
                  <th scope="col" className="num" style={{ width: "130px" }}>
                    Left
                  </th>
                  <th scope="col" style={{ width: "140px" }}>
                    <span className="sr-only">Progress</span>
                  </th>
                  <th scope="col" style={{ width: "100px" }}>
                    Roll over
                  </th>
                </tr>
              </thead>
              {byGroup.map((g) => (
                <tbody key={g.id}>
                  <tr className="budget-group">
                    <th scope="colgroup" colSpan={6}>
                      {names.get(-g.id) ?? ""}
                    </th>
                  </tr>
                  {g.lines.map((l) => (
                    <BudgetRow
                      key={l.category_id}
                      line={l}
                      name={names.get(l.category_id) ?? "?"}
                      month={month}
                      editing={editing === l.category_id}
                      onEdit={() => setEditing(l.category_id)}
                      onStopEditing={() => setEditing(null)}
                      onSave={(cents) =>
                        act(() => api.setBudget(l.category_id, month, cents), "Couldn't save the budget")
                      }
                      onUndo={() => act(() => api.clearBudget(l.category_id, month), "Couldn't undo")}
                      onRollover={(on) =>
                        act(() => api.setBudgetRollover(l.category_id, on), "Couldn't change rollover")
                      }
                    />
                  ))}
                </tbody>
              ))}
            </table>
          </div>
          )}
        </>
      )}
    </div>
  );
}

type RowProps = {
  line: BudgetLine;
  name: string;
  month: string;
  editing: boolean;
  onEdit: () => void;
  onStopEditing: () => void;
  onSave: (cents: number) => Promise<void>;
  onUndo: () => Promise<void>;
  onRollover: (on: boolean) => Promise<void>;
};

function BudgetRow({ line: l, name, month, editing, onEdit, onStopEditing, onSave, onUndo, onRollover }: RowProps) {
  const available = l.budgeted === null ? null : l.budgeted + l.carried;
  const pct = available && available > 0 ? Math.min(100, Math.max(0, (l.spent / available) * 100)) : l.spent > 0 ? 100 : 0;
  const over = l.remaining !== null && l.remaining < 0;
  const inherited = l.set_in !== null && l.set_in !== month;

  return (
    <tr>
      <td>{name}</td>
      <td className="num">
        {editing ? (
          <BudgetEditor line={l} name={name} onDone={onStopEditing} onSave={onSave} />
        ) : (
          <span className="budget-amount">
            {l.set_in === month && (
              <button
                type="button"
                className="icon-btn"
                aria-label={`Undo this month's change to ${name}`}
                title={`Undo the change made in ${formatMonth(month, true)}`}
                onClick={() => void onUndo()}
              >
                <RotateCcw size={13} aria-hidden="true" />
              </button>
            )}
            <button
              type="button"
              className={`link-btn${l.budgeted === null ? " muted" : ""}`}
              aria-label={`Edit budget for ${name}`}
              title={inherited ? `Set in ${formatMonth(l.set_in!, true)}; changes apply from ${formatMonth(month, true)} on` : undefined}
              onClick={onEdit}
            >
              {l.budgeted === null ? "Set" : formatCents(l.budgeted)}
            </button>
          </span>
        )}
      </td>
      <td className="num">
        <Money cents={l.spent} />
      </td>
      <td className="num">
        {l.remaining === null ? (
          <span className="muted">—</span>
        ) : (
          <span className="cell-stack num-stack">
            <Money cents={l.remaining} />
            {l.rollover && l.carried !== 0 && (
              <span className="cell-sub">
                {l.carried > 0 ? "+" : ""}
                {formatCents(l.carried)} carried
              </span>
            )}
          </span>
        )}
      </td>
      <td>
        {l.budgeted !== null && (
          <div
            className={`budget-bar${over ? " over" : ""}`}
            role="progressbar"
            aria-label={`${name} spent`}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(pct)}
          >
            <span style={{ width: `${pct}%` }} />
          </div>
        )}
      </td>
      <td>
        {l.budgeted !== null && (
          <input
            type="checkbox"
            aria-label={`Roll over unspent ${name}`}
            title="Carry what's left (or overspent) into next month"
            checked={l.rollover}
            onChange={(e) => void onRollover(e.currentTarget.checked)}
          />
        )}
      </td>
    </tr>
  );
}

/** Inline amount editor. Mounted only while editing, so its text starts
 *  filled in and autoFocus + select work. Enter or leaving the field saves
 *  (once); Escape cancels. */
function BudgetEditor({
  line: l,
  name,
  onDone,
  onSave,
}: {
  line: BudgetLine;
  name: string;
  onDone: () => void;
  onSave: (cents: number) => Promise<void>;
}) {
  const original = l.budgeted === null ? "" : plainCents(l.budgeted);
  const [text, setText] = useState(original);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  // Set once Enter/Escape/a save has handled this edit, so a later blur
  // (e.g. when the field unmounts) doesn't act again.
  const handled = useRef(false);

  async function commit() {
    if (saving || handled.current) return;
    const trimmed = text.trim();
    if (trimmed === original) {
      handled.current = true;
      onDone();
      return;
    }
    setSaving(true);
    try {
      const cents = trimmed === "" ? 0 : await api.parseMoney(trimmed);
      if (cents < 0) {
        setError("A budget can't be negative.");
        return;
      }
      handled.current = true;
      await onSave(cents);
      onDone();
    } catch (e) {
      handled.current = false;
      setError(errorMessage(e));
    } finally {
      setSaving(false);
    }
  }

  function onKey(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter") {
      e.preventDefault();
      void commit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      handled.current = true;
      onDone();
    }
  }

  const errId = `budget-err-${l.category_id}`;
  return (
    <div className="budget-edit">
      <input
        aria-label={`Budget for ${name}`}
        aria-invalid={error ? true : undefined}
        aria-describedby={error ? errId : undefined}
        inputMode="decimal"
        autoFocus
        onFocus={(e) => e.currentTarget.select()}
        value={text}
        disabled={saving}
        onChange={(e) => setText(e.currentTarget.value)}
        onKeyDown={onKey}
        onBlur={() => void commit()}
        placeholder="0.00"
      />
      {error && (
        <span id={errId} className="field-error">
          {error}
        </span>
      )}
    </div>
  );
}
