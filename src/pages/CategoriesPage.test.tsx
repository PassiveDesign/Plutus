import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../components";
import { api, CategoryGroup, Rule } from "../lib/api";
import { CategoriesPage } from "./CategoriesPage";

vi.mock("../lib/api", async (orig) => {
  const real = await orig<typeof import("../lib/api")>();
  return {
    ...real,
    api: {
      ...real.api,
      listCategories: vi.fn(),
      categoryUsage: vi.fn(),
      createCategoryGroup: vi.fn(),
      updateCategoryGroup: vi.fn(),
      setCategoryGroupArchived: vi.fn(),
      createCategory: vi.fn(),
      updateCategory: vi.fn(),
      setCategoryArchived: vi.fn(),
      listRules: vi.fn(),
      createRule: vi.fn(),
      updateRule: vi.fn(),
      deleteRule: vi.fn(),
      moveRule: vi.fn(),
      applyRules: vi.fn(),
    },
  };
});
const m = vi.mocked(api);

let tree: CategoryGroup[];
let rules: Rule[];

beforeEach(() => {
  vi.resetAllMocks();
  tree = [
    {
      id: 1, name: "Income", kind: "income", sort_order: 1, archived_at: null,
      categories: [{ id: 1, group_id: 1, name: "Salary & wages", sort_order: 1, archived_at: null }],
    },
    {
      id: 2, name: "Food", kind: "expense", sort_order: 2, archived_at: null,
      categories: [
        { id: 10, group_id: 2, name: "Groceries", sort_order: 1, archived_at: null },
        { id: 11, group_id: 2, name: "Old snacks", sort_order: 2, archived_at: "z" },
      ],
    },
    {
      id: 3, name: "Retired", kind: "expense", sort_order: 3, archived_at: "z",
      categories: [{ id: 30, group_id: 3, name: "Smokes", sort_order: 1, archived_at: null }],
    },
  ];
  m.listCategories.mockImplementation(async () => tree);
  rules = [
    { id: 1, pattern: "woolworths", match_field: "either", direction: null, category_id: 10, sort_order: 1 },
    { id: 2, pattern: "acme", match_field: "description", direction: "in", category_id: 1, sort_order: 2 },
  ];
  m.listRules.mockImplementation(async () => rules);
  m.moveRule.mockImplementation(async (id, up) => {
    const i = rules.findIndex((r) => r.id === id);
    const j = up ? i - 1 : i + 1;
    [rules[i], rules[j]] = [rules[j], rules[i]];
    return [...rules];
  });
  m.createRule.mockImplementation(async (input) => {
    const r = { ...input, id: 9, sort_order: 9 };
    rules = [...rules, r];
    return r;
  });
  m.deleteRule.mockImplementation(async (id) => {
    rules = rules.filter((r) => r.id !== id);
  });
  m.applyRules.mockResolvedValue(3);
  m.categoryUsage.mockResolvedValue({ "10": 12, "1": 1 });
  m.createCategory.mockImplementation(async (i) => ({ id: 99, group_id: i.group_id, name: i.name, sort_order: 9, archived_at: null }));
  m.updateCategory.mockImplementation(async (id, i) => ({ id, group_id: i.group_id, name: i.name, sort_order: 9, archived_at: null }));
  m.createCategoryGroup.mockImplementation(async (i) => ({ id: 50, ...i, sort_order: 9, archived_at: null, categories: [] }));
  m.setCategoryArchived.mockImplementation(async (id) => tree[1].categories.find((c) => c.id === id)!);
  m.setCategoryGroupArchived.mockImplementation(async (id) => tree.find((g) => g.id === id)!);
});

function renderPage() {
  const user = userEvent.setup();
  render(
    <ToastProvider>
      <CategoriesPage />
    </ToastProvider>,
  );
  return user;
}

describe("CategoriesPage", () => {
  it("lists active groups with categories, kinds and usage", async () => {
    renderPage();
    const food = await screen.findByRole("region", { name: "Food" });
    expect(screen.getByRole("region", { name: "Income" })).toHaveTextContent("Income");
    expect(within(food).getByText("Expenses")).toBeInTheDocument();
    expect(within(food).getByText("Groceries").closest("li")).toHaveTextContent("12 transactions");
    // Wait for the Rules list too, so the text appears in both places
    // (previously this raced on fast machines).
    await within(screen.getByRole("region", { name: "Rules" })).findAllByRole("listitem");
    const income = screen.getByRole("region", { name: "Income" });
    expect(within(income).getByText("Salary & wages").closest("li")).toHaveTextContent("1 transaction");
    expect(within(food).queryByText("Old snacks")).toBeNull();
    expect(screen.queryByRole("region", { name: "Retired" })).toBeNull();
  });

  it("show archived reveals archived groups and categories", async () => {
    const user = renderPage();
    await user.click(await screen.findByLabelText("Show archived (2)"));
    expect(screen.getByRole("region", { name: "Retired" })).toHaveTextContent("Archived");
    expect(screen.getByText("Old snacks").closest("li")).toHaveTextContent("Unused");
    await user.click(screen.getByRole("button", { name: "Restore Old snacks" }));
    expect(m.setCategoryArchived).toHaveBeenCalledWith(11, false);
    await user.click(screen.getByRole("button", { name: "Restore group Retired" }));
    expect(m.setCategoryGroupArchived).toHaveBeenCalledWith(3, false);
  });

  it("adds a category to a group", async () => {
    const user = renderPage();
    const food = await screen.findByRole("region", { name: "Food" });
    await user.click(within(food).getByRole("button", { name: "Add category" }));
    const dialog = screen.getByRole("dialog", { name: "Add category" });
    expect(within(dialog).getByLabelText("Group")).toHaveValue("2");
    await user.type(within(dialog).getByLabelText("Name"), "Snacks");
    await user.click(within(dialog).getByRole("button", { name: "Add category" }));
    expect(m.createCategory).toHaveBeenCalledWith({ group_id: 2, name: "Snacks" });
    expect(await screen.findByRole("status")).toHaveTextContent("Category Snacks added.");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("renames and moves a category; archived groups aren't offered", async () => {
    const user = renderPage();
    await user.click(await screen.findByRole("button", { name: "Edit Groceries" }));
    const dialog = screen.getByRole("dialog", { name: "Edit category" });
    const group = within(dialog).getByLabelText("Group");
    expect(within(group).queryByText(/Retired/)).toBeNull();
    await user.selectOptions(group, "1");
    await user.clear(within(dialog).getByLabelText("Name"));
    await user.type(within(dialog).getByLabelText("Name"), "Supermarket");
    await user.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(m.updateCategory).toHaveBeenCalledWith(10, { group_id: 1, name: "Supermarket" });
  });

  it("name clashes show under the field", async () => {
    m.createCategory.mockRejectedValueOnce({
      code: "conflict",
      field: "name",
      message: 'This group already has a category named "Groceries".',
    });
    const user = renderPage();
    const food = await screen.findByRole("region", { name: "Food" });
    await user.click(within(food).getByRole("button", { name: "Add category" }));
    const dialog = screen.getByRole("dialog");
    await user.type(within(dialog).getByLabelText("Name"), "groceries");
    await user.click(within(dialog).getByRole("button", { name: "Add category" }));
    expect(within(dialog).getByLabelText("Name")).toHaveAccessibleDescription(
      'This group already has a category named "Groceries".',
    );
  });

  it("adds an income group", async () => {
    const user = renderPage();
    await user.click(await screen.findByRole("button", { name: "Add group" }));
    const dialog = screen.getByRole("dialog", { name: "Add group" });
    expect(within(dialog).getByLabelText("Expenses")).toBeChecked();
    await user.type(within(dialog).getByLabelText("Name"), "Side hustle");
    await user.click(within(dialog).getByLabelText("Income"));
    await user.click(within(dialog).getByRole("button", { name: "Add group" }));
    expect(m.createCategoryGroup).toHaveBeenCalledWith({ name: "Side hustle", kind: "income" });
  });

  it("archives a category", async () => {
    const user = renderPage();
    await user.click(await screen.findByRole("button", { name: "Archive Groceries" }));
    expect(m.setCategoryArchived).toHaveBeenCalledWith(10, true);
    expect(await screen.findByRole("status")).toHaveTextContent("Groceries archived.");
  });

  it("lists rules in order in plain English", async () => {
    renderPage();
    const section = await screen.findByRole("region", { name: "Rules" });
    const items = await within(section).findAllByRole("listitem");
    expect(items[0]).toHaveTextContent("Payee or description contains “woolworths” → Groceries");
    expect(items[1]).toHaveTextContent("Bank description contains “acme” (money in) → Salary & wages");
    expect(within(items[0]).getByRole("button", { name: "Move up: woolworths" })).toBeDisabled();
  });

  it("reorders rules", async () => {
    const user = renderPage();
    await user.click(await screen.findByRole("button", { name: "Move down: woolworths" }));
    expect(m.moveRule).toHaveBeenCalledWith(1, false);
    const items = within(screen.getByRole("region", { name: "Rules" })).getAllByRole("listitem");
    expect(items[0]).toHaveTextContent("acme");
  });

  it("adds a rule with a category; requires a category", async () => {
    const user = renderPage();
    const section = await screen.findByRole("region", { name: "Rules" });
    await user.click(within(section).getByRole("button", { name: "Add rule" }));
    const dialog = screen.getByRole("dialog", { name: "Add rule" });
    await user.type(within(dialog).getByLabelText("Contains"), "Shell");
    await user.click(within(dialog).getByRole("button", { name: "Add rule" }));
    expect(within(dialog).getByLabelText("Category")).toHaveAccessibleDescription("Choose a category.");
    expect(m.createRule).not.toHaveBeenCalled();
    await user.selectOptions(within(dialog).getByLabelText("Money"), "out");
    await user.selectOptions(within(dialog).getByLabelText("Look in"), "payee");
    await user.selectOptions(within(dialog).getByLabelText("Category"), "10");
    await user.click(within(dialog).getByRole("button", { name: "Add rule" }));
    expect(m.createRule).toHaveBeenCalledWith({ pattern: "Shell", match_field: "payee", direction: "out", category_id: 10 });
    expect(await screen.findByText(/Payee contains “Shell” \(money out\)/)).toBeInTheDocument();
  });

  it("deletes a rule after confirming", async () => {
    const user = renderPage();
    await user.click(await screen.findByRole("button", { name: "Edit rule: acme" }));
    const dialog = screen.getByRole("dialog", { name: "Edit rule" });
    await user.click(within(dialog).getByRole("button", { name: "Delete" }));
    expect(m.deleteRule).not.toHaveBeenCalled();
    await user.click(within(dialog).getByRole("button", { name: "Really delete?" }));
    expect(m.deleteRule).toHaveBeenCalledWith(2);
  });

  it("applies rules to uncategorised and refreshes usage", async () => {
    const user = renderPage();
    const section = await screen.findByRole("region", { name: "Rules" });
    await screen.findAllByText(/woolworths/);
    m.categoryUsage.mockClear();
    await user.click(within(section).getByRole("button", { name: "Apply to uncategorised" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Categorised 3 transactions.");
    expect(m.categoryUsage).toHaveBeenCalled();
  });
});
