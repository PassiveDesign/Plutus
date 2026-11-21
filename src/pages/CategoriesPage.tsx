import { useCallback, useEffect, useState } from "react";
import { Archive, ArchiveRestore, Pencil, Plus } from "lucide-react";
import { Button, useToast } from "../components";
import { api, Category, CategoryGroup, errorMessage } from "../lib/api";
import { CategoryForm, GroupForm } from "./CategoryForms";
import { RulesSection } from "./RulesSection";

type Editing =
  | { kind: "group"; group: CategoryGroup | null }
  | { kind: "category"; category: Category | null; groupId: number };

export function CategoriesPage() {
  const toast = useToast();
  const [groups, setGroups] = useState<CategoryGroup[] | null>(null);
  const [usage, setUsage] = useState<Record<string, number>>({});
  const [loadError, setLoadError] = useState<string | null>(null);
  const [showArchived, setShowArchived] = useState(false);
  const [editing, setEditing] = useState<Editing | null>(null);

  const load = useCallback(async () => {
    try {
      const [tree, counts] = await Promise.all([api.listCategories(true), api.categoryUsage()]);
      setGroups(tree);
      setUsage(counts);
      setLoadError(null);
    } catch (e) {
      setLoadError(errorMessage(e));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  function done(message: string) {
    setEditing(null);
    toast.success(message);
    void load();
  }

  async function archiveGroup(g: CategoryGroup, archived: boolean) {
    try {
      await api.setCategoryGroupArchived(g.id, archived);
      toast.success(archived ? `${g.name} archived.` : `${g.name} restored.`);
      void load();
    } catch (e) {
      toast.error(errorMessage(e));
    }
  }

  async function archiveCategory(c: Category, archived: boolean) {
    try {
      await api.setCategoryArchived(c.id, archived);
      toast.success(archived ? `${c.name} archived.` : `${c.name} restored.`);
      void load();
    } catch (e) {
      toast.error(errorMessage(e));
    }
  }

  if (loadError) {
    return (
      <div className="page-error" role="alert">
        <p>Couldn't load categories: {loadError}</p>
        <Button variant="secondary" onClick={() => void load()}>
          Try again
        </Button>
      </div>
    );
  }
  if (groups === null) return <div aria-busy="true" />;

  const archivedCount =
    groups.filter((g) => g.archived_at).length +
    groups.flatMap((g) => g.categories).filter((c) => c.archived_at).length;
  const shown = groups.filter((g) => showArchived || g.archived_at === null);

  return (
    <div className="categories">
      <div className="toolbar">
        <p className="muted categories-intro">
          Archiving hides a category from pickers; transactions keep it. Nothing is ever deleted.
        </p>
        <span className="spacer" />
        {archivedCount > 0 && (
          <label className="check">
            <input type="checkbox" checked={showArchived} onChange={(e) => setShowArchived(e.currentTarget.checked)} />
            Show archived ({archivedCount})
          </label>
        )}
        <Button onClick={() => setEditing({ kind: "group", group: null })}>
          <Plus size={16} aria-hidden="true" /> Add group
        </Button>
      </div>

      <RulesSection groups={groups} onApplied={() => void load()} />

      {shown.map((g) => {
        const cats = g.categories.filter((c) => showArchived || c.archived_at === null);
        const groupArchived = g.archived_at !== null;
        return (
          <section
            key={g.id}
            className={`category-group${groupArchived ? " archived" : ""}`}
            aria-labelledby={`group-${g.id}`}
          >
            <header className="category-group-head">
              <h2 id={`group-${g.id}`}>{g.name}</h2>
              <span className={`badge kind-${g.kind}`}>{g.kind === "income" ? "Income" : "Expenses"}</span>
              {groupArchived && <span className="badge">Archived</span>}
              <span className="spacer" />
              {!groupArchived && (
                <Button variant="ghost" onClick={() => setEditing({ kind: "category", category: null, groupId: g.id })}>
                  <Plus size={14} aria-hidden="true" /> Add category
                </Button>
              )}
              <Button variant="ghost" aria-label={`Edit group ${g.name}`} onClick={() => setEditing({ kind: "group", group: g })}>
                <Pencil size={14} aria-hidden="true" />
              </Button>
              <Button
                variant="ghost"
                aria-label={`${groupArchived ? "Restore" : "Archive"} group ${g.name}`}
                title={groupArchived ? "Restore group" : "Archive group (hides all its categories)"}
                onClick={() => void archiveGroup(g, !groupArchived)}
              >
                {groupArchived ? <ArchiveRestore size={14} aria-hidden="true" /> : <Archive size={14} aria-hidden="true" />}
              </Button>
            </header>
            {cats.length === 0 ? (
              <p className="muted category-empty">No categories in this group.</p>
            ) : (
              <ul className="category-list">
                {cats.map((c) => {
                  const n = usage[String(c.id)] ?? 0;
                  const archived = c.archived_at !== null;
                  return (
                    <li key={c.id} className={archived ? "archived" : undefined}>
                      <span className="category-name">{c.name}</span>
                      {archived && <span className="badge">Archived</span>}
                      <span className="muted category-count">
                        {n === 0 ? "Unused" : `${n} transaction${n === 1 ? "" : "s"}`}
                      </span>
                      <Button
                        variant="ghost"
                        aria-label={`Edit ${c.name}`}
                        onClick={() => setEditing({ kind: "category", category: c, groupId: g.id })}
                      >
                        <Pencil size={14} aria-hidden="true" />
                      </Button>
                      <Button
                        variant="ghost"
                        aria-label={`${archived ? "Restore" : "Archive"} ${c.name}`}
                        onClick={() => void archiveCategory(c, !archived)}
                      >
                        {archived ? <ArchiveRestore size={14} aria-hidden="true" /> : <Archive size={14} aria-hidden="true" />}
                      </Button>
                    </li>
                  );
                })}
              </ul>
            )}
          </section>
        );
      })}

      {editing?.kind === "group" && (
        <GroupForm group={editing.group} onClose={() => setEditing(null)} onDone={done} />
      )}
      {editing?.kind === "category" && (
        <CategoryForm
          category={editing.category}
          groupId={editing.groupId}
          groups={groups}
          onClose={() => setEditing(null)}
          onDone={done}
        />
      )}
    </div>
  );
}
