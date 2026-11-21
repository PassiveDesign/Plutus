import { FormEvent, useState } from "react";
import { Button, Modal, SelectField, TextField, useToast } from "../components";
import { api, Category, CategoryGroup, GroupKind, splitError } from "../lib/api";

type Done = (message: string) => void;

function useSave(fallback: string) {
  const toast = useToast();
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  async function run(fields: string[], action: () => Promise<void>) {
    if (busy) return;
    setBusy(true);
    setErrors({});
    try {
      await action();
    } catch (e) {
      const { field, general } = splitError(e);
      const inline = Object.fromEntries(Object.entries(field).filter(([k]) => fields.includes(k)));
      if (Object.keys(inline).length > 0) setErrors(inline);
      else toast.error(general ?? Object.values(field)[0] ?? fallback);
      setBusy(false);
    }
  }
  return { errors, busy, run };
}

/** Add or edit a category group (name + income/expense). */
export function GroupForm({ group, onClose, onDone }: { group: CategoryGroup | null; onClose: () => void; onDone: Done }) {
  const [name, setName] = useState(group?.name ?? "");
  const [kind, setKind] = useState<GroupKind>(group?.kind ?? "expense");
  const { errors, busy, run } = useSave("Couldn't save the group.");

  function save(e: FormEvent) {
    e.preventDefault();
    void run(["name"], async () => {
      const g = group ? await api.updateCategoryGroup(group.id, { name, kind }) : await api.createCategoryGroup({ name, kind });
      onDone(group ? `Group ${g.name} saved.` : `Group ${g.name} added.`);
    });
  }

  return (
    <Modal
      open
      title={group ? "Edit group" : "Add group"}
      onClose={onClose}
      footer={
        <>
          <span className="spacer" />
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button type="submit" form="group-form" busy={busy} busyLabel="Saving…">
            {group ? "Save" : "Add group"}
          </Button>
        </>
      }
    >
      <form id="group-form" className="form-grid" onSubmit={save} noValidate>
        <TextField label="Name" value={name} onChange={(e) => setName(e.currentTarget.value)} error={errors.name} autoFocus />
        <fieldset className="radio-row">
          <legend>Type</legend>
          <div className="radio-options">
            {(["expense", "income"] as const).map((k) => (
              <label key={k} className="check">
                <input type="radio" name="group-kind" checked={kind === k} onChange={() => setKind(k)} />
                {k === "expense" ? "Expenses" : "Income"}
              </label>
            ))}
          </div>
        </fieldset>
      </form>
    </Modal>
  );
}

/** Add a category to a group, or rename / move one. */
export function CategoryForm({
  category,
  groupId,
  groups,
  onClose,
  onDone,
}: {
  category: Category | null;
  /** Group for a new category. */
  groupId: number;
  /** Full tree; only active groups (plus the current one) are offered. */
  groups: CategoryGroup[];
  onClose: () => void;
  onDone: Done;
}) {
  const [name, setName] = useState(category?.name ?? "");
  const [group, setGroup] = useState(category?.group_id ?? groupId);
  const { errors, busy, run } = useSave("Couldn't save the category.");
  const choices = groups.filter((g) => g.archived_at === null || g.id === (category?.group_id ?? groupId));

  function save(e: FormEvent) {
    e.preventDefault();
    void run(["name", "group_id"], async () => {
      const input = { group_id: group, name };
      const c = category ? await api.updateCategory(category.id, input) : await api.createCategory(input);
      onDone(category ? `Category ${c.name} saved.` : `Category ${c.name} added.`);
    });
  }

  return (
    <Modal
      open
      title={category ? "Edit category" : "Add category"}
      onClose={onClose}
      footer={
        <>
          <span className="spacer" />
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button type="submit" form="category-form" busy={busy} busyLabel="Saving…">
            {category ? "Save" : "Add category"}
          </Button>
        </>
      }
    >
      <form id="category-form" className="form-grid" onSubmit={save} noValidate>
        <TextField label="Name" value={name} onChange={(e) => setName(e.currentTarget.value)} error={errors.name} autoFocus />
        <SelectField
          label="Group"
          value={String(group)}
          onChange={(e) => setGroup(Number(e.currentTarget.value))}
          error={errors.group_id}
          hint={category ? "Moving a category keeps its transactions." : undefined}
        >
          {choices.map((g) => (
            <option key={g.id} value={g.id}>
              {g.name}
              {g.archived_at ? " (archived)" : ""}
            </option>
          ))}
        </SelectField>
      </form>
    </Modal>
  );
}
