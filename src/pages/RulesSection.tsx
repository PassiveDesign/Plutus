import { FormEvent, useCallback, useEffect, useState } from "react";
import { ArrowDown, ArrowUp, Pencil, Plus, Wand2 } from "lucide-react";
import { Button, Modal, SelectField, TextField, useToast } from "../components";
import {
  api,
  CategoryGroup,
  errorMessage,
  MatchField,
  Rule,
  RuleDirection,
  RuleInput,
  splitError,
} from "../lib/api";
import { categoryIndex, CategorySelect } from "./CategorySelect";

const FIELD_TEXT: Record<MatchField, string> = {
  either: "Payee or description",
  payee: "Payee",
  description: "Bank description",
};

/** Plain-English summary, e.g. "Payee contains “shell” (money out)". */
export function describeRule(r: RuleInput): string {
  const dir = r.direction === "in" ? " (money in)" : r.direction === "out" ? " (money out)" : "";
  return `${FIELD_TEXT[r.match_field]} contains “${r.pattern}”${dir}`;
}

type Props = {
  /** Full category tree (archived included). */
  groups: CategoryGroup[];
  /** Called after rules categorise transactions (usage counts change). */
  onApplied: () => void;
};

export function RulesSection({ groups, onApplied }: Props) {
  const toast = useToast();
  const [rules, setRules] = useState<Rule[] | null>(null);
  const [editing, setEditing] = useState<Rule | "new" | null>(null);
  const [applying, setApplying] = useState(false);
  const cats = categoryIndex(groups);

  const load = useCallback(async () => {
    try {
      setRules(await api.listRules());
    } catch (e) {
      toast.error(`Couldn't load rules: ${errorMessage(e)}`);
    }
  }, [toast]);

  useEffect(() => {
    void load();
  }, [load]);

  async function move(r: Rule, up: boolean) {
    try {
      setRules(await api.moveRule(r.id, up));
    } catch (e) {
      toast.error(errorMessage(e));
    }
  }

  async function apply() {
    setApplying(true);
    try {
      const n = await api.applyRules();
      toast.success(
        n === 0 ? "No uncategorised transactions matched a rule." : `Categorised ${n} transaction${n === 1 ? "" : "s"}.`,
      );
      if (n > 0) onApplied();
    } catch (e) {
      toast.error(`Couldn't apply rules: ${errorMessage(e)}`);
    } finally {
      setApplying(false);
    }
  }

  function done(message: string) {
    setEditing(null);
    toast.success(message);
    void load();
  }

  return (
    <section className="category-group rules" aria-labelledby="rules-heading">
      <header className="category-group-head">
        <h2 id="rules-heading">Rules</h2>
        <span className="spacer" />
        <Button variant="ghost" onClick={() => setEditing("new")}>
          <Plus size={14} aria-hidden="true" /> Add rule
        </Button>
        <Button
          variant="secondary"
          onClick={() => void apply()}
          disabled={!rules || rules.length === 0}
          busy={applying}
          busyLabel="Applying…"
        >
          <Wand2 size={14} aria-hidden="true" /> Apply to uncategorised
        </Button>
      </header>
      <p className="muted category-empty">
        Imported transactions are categorised by the first matching rule. Rules never replace a category you set.
      </p>
      {rules && rules.length > 0 && (
        <ol className="category-list rule-list">
          {rules.map((r, i) => {
            const c = cats.get(r.category_id);
            return (
              <li key={r.id}>
                <span className="category-name">
                  {describeRule(r)} → <strong>{c?.name ?? "?"}</strong>
                </span>
                <Button variant="ghost" aria-label={`Move up: ${r.pattern}`} disabled={i === 0} onClick={() => void move(r, true)}>
                  <ArrowUp size={14} aria-hidden="true" />
                </Button>
                <Button
                  variant="ghost"
                  aria-label={`Move down: ${r.pattern}`}
                  disabled={i === rules.length - 1}
                  onClick={() => void move(r, false)}
                >
                  <ArrowDown size={14} aria-hidden="true" />
                </Button>
                <Button variant="ghost" aria-label={`Edit rule: ${r.pattern}`} onClick={() => setEditing(r)}>
                  <Pencil size={14} aria-hidden="true" />
                </Button>
              </li>
            );
          })}
        </ol>
      )}
      {editing !== null && (
        <RuleForm rule={editing === "new" ? null : editing} groups={groups} onClose={() => setEditing(null)} onDone={done} />
      )}
    </section>
  );
}

export function RuleForm({
  rule,
  groups,
  initial,
  onClose,
  onDone,
}: {
  rule: Rule | null;
  groups: CategoryGroup[];
  /** Prefill for a new rule. */
  initial?: Partial<RuleInput>;
  onClose: () => void;
  onDone: (message: string) => void;
}) {
  const toast = useToast();
  const [pattern, setPattern] = useState(rule?.pattern ?? initial?.pattern ?? "");
  const [field, setField] = useState<MatchField>(rule?.match_field ?? initial?.match_field ?? "either");
  const [direction, setDirection] = useState<RuleDirection | null>(rule?.direction ?? initial?.direction ?? null);
  const [categoryId, setCategoryId] = useState<number | null>(rule?.category_id ?? initial?.category_id ?? null);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  async function save(e: FormEvent) {
    e.preventDefault();
    if (busy) return;
    if (categoryId === null) {
      setErrors({ category_id: "Choose a category." });
      return;
    }
    setBusy(true);
    setErrors({});
    const input: RuleInput = { pattern, match_field: field, direction, category_id: categoryId };
    try {
      if (rule) await api.updateRule(rule.id, input);
      else await api.createRule(input);
      onDone(rule ? "Rule saved." : "Rule added.");
    } catch (err) {
      const { field: f, general } = splitError(err);
      if (f.pattern || f.category_id) setErrors(f);
      else toast.error(general ?? "Couldn't save the rule.");
      setBusy(false);
    }
  }

  async function remove() {
    if (!rule || busy) return;
    if (!confirmDelete) {
      setConfirmDelete(true);
      return;
    }
    setBusy(true);
    try {
      await api.deleteRule(rule.id);
      onDone("Rule deleted.");
    } catch (err) {
      toast.error(errorMessage(err));
      setBusy(false);
    }
  }

  return (
    <Modal
      open
      title={rule ? "Edit rule" : "Add rule"}
      onClose={onClose}
      footer={
        <>
          {rule && (
            <Button variant={confirmDelete ? "danger" : "ghost"} onClick={() => void remove()} disabled={busy}>
              {confirmDelete ? "Really delete?" : "Delete"}
            </Button>
          )}
          <span className="spacer" />
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button type="submit" form="rule-form" busy={busy} busyLabel="Saving…">
            {rule ? "Save" : "Add rule"}
          </Button>
        </>
      }
    >
      <form id="rule-form" className="form-grid" onSubmit={save} noValidate>
        <div className="form-row">
          <SelectField label="Look in" value={field} onChange={(e) => setField(e.currentTarget.value as MatchField)}>
            <option value="either">Payee or description</option>
            <option value="payee">Payee</option>
            <option value="description">Bank description</option>
          </SelectField>
          <SelectField
            label="Money"
            value={direction ?? ""}
            onChange={(e) => setDirection(e.currentTarget.value === "" ? null : (e.currentTarget.value as RuleDirection))}
          >
            <option value="">In or out</option>
            <option value="out">Money out only</option>
            <option value="in">Money in only</option>
          </SelectField>
        </div>
        <TextField
          label="Contains"
          value={pattern}
          onChange={(e) => setPattern(e.currentTarget.value)}
          error={errors.pattern}
          hint="Not case-sensitive, e.g. woolworths"
          autoFocus
        />
        <CategorySelect
          label="Category"
          groups={groups}
          value={categoryId}
          onChange={setCategoryId}
          keep={rule?.category_id ?? null}
          error={errors.category_id}
          noneLabel="Choose a category"
        />
      </form>
    </Modal>
  );
}
