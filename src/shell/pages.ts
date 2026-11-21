import type { LucideIcon } from "lucide-react";
import {
  ArrowLeftRight,
  ChartPie,
  FileUp,
  LayoutDashboard,
  PiggyBank,
  Settings,
  Wallet,
} from "lucide-react";

export type PageId =
  | "dashboard"
  | "accounts"
  | "transactions"
  | "import"
  | "budgets"
  | "reports"
  | "settings";

export type PageDef = {
  id: PageId;
  label: string;
  icon: LucideIcon;
  /** One-line description shown on the placeholder until the page is built. */
  summary: string;
  /** Roadmap milestone that delivers the page. */
  milestone: string;
};

/** Sidebar order. Settings is pinned to the bottom separately. */
export const PAGES: PageDef[] = [
  {
    id: "dashboard",
    label: "Dashboard",
    icon: LayoutDashboard,
    summary: "Net worth, this month's spending and account balances at a glance.",
    milestone: "M6",
  },
  {
    id: "accounts",
    label: "Accounts",
    icon: Wallet,
    summary: "Your bank accounts, cards and loans with their balances.",
    milestone: "M1",
  },
  {
    id: "transactions",
    label: "Transactions",
    icon: ArrowLeftRight,
    summary: "Every transaction, with search, filters, categories and imports.",
    milestone: "M2",
  },
  {
    id: "import",
    label: "Import",
    icon: FileUp,
    summary: "Bring in transactions from your bank's CSV export.",
    milestone: "M4",
  },
  {
    id: "budgets",
    label: "Budgets",
    icon: PiggyBank,
    summary: "Monthly budgets per category: budgeted, spent and remaining.",
    milestone: "M5",
  },
  {
    id: "reports",
    label: "Reports",
    icon: ChartPie,
    summary: "Spending by category, income vs expenses, net worth over time.",
    milestone: "M6",
  },
];

export const SETTINGS_PAGE: PageDef = {
  id: "settings",
  label: "Settings",
  icon: Settings,
  summary: "Password, backups and preferences.",
  milestone: "M0/M7",
};

export const ALL_PAGES: PageDef[] = [...PAGES, SETTINGS_PAGE];

export function pageById(id: PageId): PageDef {
  return ALL_PAGES.find((p) => p.id === id) ?? PAGES[0];
}
