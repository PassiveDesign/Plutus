import { SelectField } from "../components";
import type { CategoryGroup } from "../lib/api";

type Props = {
  label?: string;
  /** Full tree including archived items (to label a kept archived category). */
  groups: CategoryGroup[];
  value: number | null;
  onChange: (id: number | null) => void;
  /** An archived category that may stay selected (the transaction's own). */
  keep?: number | null;
  error?: string;
  disabled?: boolean;
  /** Text for the empty choice. */
  noneLabel?: string;
};

/** Category picker grouped by category group; active items only, plus `keep`. */
export function CategorySelect({
  label = "Category",
  groups,
  value,
  onChange,
  keep = null,
  error,
  disabled,
  noneLabel = "Uncategorised",
}: Props) {
  const shown = groups
    .map((g) => ({
      ...g,
      categories: g.categories.filter(
        (c) => (c.archived_at === null && g.archived_at === null) || c.id === keep,
      ),
    }))
    .filter((g) => g.categories.length > 0);
  return (
    <SelectField
      label={label}
      value={value === null ? "" : String(value)}
      onChange={(e) => onChange(e.currentTarget.value === "" ? null : Number(e.currentTarget.value))}
      error={error}
      disabled={disabled}
    >
      <option value="">{noneLabel}</option>
      {shown.map((g) => (
        <optgroup key={g.id} label={g.name}>
          {g.categories.map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
              {c.archived_at || g.archived_at ? " (archived)" : ""}
            </option>
          ))}
        </optgroup>
      ))}
    </SelectField>
  );
}

/** id → { name, group } for display. */
export function categoryIndex(groups: CategoryGroup[]): Map<number, { name: string; group: string }> {
  const m = new Map<number, { name: string; group: string }>();
  for (const g of groups) for (const c of g.categories) m.set(c.id, { name: c.name, group: g.name });
  return m;
}
