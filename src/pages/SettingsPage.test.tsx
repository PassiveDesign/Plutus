import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../components";
import { api } from "../lib/api";
import { SettingsPage } from "./SettingsPage";

vi.mock("../lib/api", async (orig) => {
  const real = await orig<typeof import("../lib/api")>();
  return {
    ...real,
    api: {
      ...real.api,
      changePassword: vi.fn(),
      backupFolder: vi.fn(),
      listBackups: vi.fn(),
      backupNow: vi.fn(),
      restoreBackup: vi.fn(),
    },
  };
});
const changePassword = vi.mocked(api.changePassword);
const backupFolder = vi.mocked(api.backupFolder);
const listBackups = vi.mocked(api.listBackups);
const backupNow = vi.mocked(api.backupNow);
const restoreBackup = vi.mocked(api.restoreBackup);

const backup = (name: string, bytes = 1024, modified = "2026-11-21T09:30:00+11:00") => ({
  name,
  path: `/home/me/.local/share/plutus/${name}`,
  bytes,
  modified,
});

beforeEach(() => {
  changePassword.mockReset();
  backupFolder.mockReset().mockResolvedValue("/home/me/.local/share/plutus/backups");
  listBackups.mockReset().mockResolvedValue([]);
  backupNow.mockReset();
  restoreBackup.mockReset();
});

function setup() {
  render(
    <ToastProvider>
      <SettingsPage minChars={8} />
    </ToastProvider>,
  );
  const user = userEvent.setup();
  const f = {
    current: screen.getByLabelText("Current password"),
    next: screen.getByLabelText("New password"),
    confirm: screen.getByLabelText("Confirm new password"),
    submit: screen.getByRole("button", { name: "Change password" }),
  };
  return { user, f };
}

describe("SettingsPage: change password", () => {
  it("validates length, sameness and confirmation before enabling submit", async () => {
    const { user, f } = setup();
    expect(f.submit).toBeDisabled();
    await user.type(f.current, "old password");
    await user.type(f.next, "short");
    expect(f.next).toHaveAccessibleDescription("At least 8 characters (5/8).");
    await user.clear(f.next);
    await user.type(f.next, "old password");
    expect(f.next).toHaveAccessibleDescription(
      "New password must be different from the current one.",
    );
    await user.clear(f.next);
    await user.type(f.next, "new password");
    await user.type(f.confirm, "new passwordX");
    expect(f.confirm).toHaveAccessibleDescription("Passwords don't match.");
    expect(f.submit).toBeDisabled();
    await user.type(f.confirm, "{Backspace}");
    expect(f.submit).toBeEnabled();
  });

  it("changes the password, clears the form and confirms", async () => {
    changePassword.mockResolvedValue(undefined);
    const { user, f } = setup();
    await user.type(f.current, "old password");
    await user.type(f.next, "new password");
    await user.type(f.confirm, "new password");
    await user.click(f.submit);
    expect(changePassword).toHaveBeenCalledWith("old password", "new password");
    expect(await screen.findByRole("status")).toHaveTextContent("Password changed.");
    expect(f.current).toHaveValue("");
    expect(f.next).toHaveValue("");
    expect(f.confirm).toHaveValue("");
  });

  it("wrong current password: inline error, refocus, new password kept", async () => {
    let reject!: (e: unknown) => void;
    changePassword.mockImplementation(() => new Promise((_, r) => (reject = r)));
    const { user, f } = setup();
    await user.type(f.current, "not it");
    await user.type(f.next, "new password");
    await user.type(f.confirm, "new password");
    await user.click(f.submit);
    expect(f.current).toBeDisabled();
    f.current.blur(); // browsers drop focus from disabled inputs; jsdom doesn't
    reject({ code: "wrong_password", message: "incorrect password" });

    expect(await screen.findByText("Current password is incorrect.")).toBeInTheDocument();
    expect(f.current).toHaveValue("");
    expect(f.next).toHaveValue("new password");
    await vi.waitFor(() => expect(f.current).toHaveFocus());
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("other failures show an error toast", async () => {
    changePassword.mockRejectedValue({ code: "io", message: "i/o error: disk full" });
    const { user, f } = setup();
    await user.type(f.current, "old password");
    await user.type(f.next, "new password");
    await user.type(f.confirm, "new password");
    await user.click(f.submit);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Couldn't change password: I/o error: disk full.",
    );
  });
});

describe("SettingsPage: backup", () => {
  function setupBackup(onDataReplaced = vi.fn()) {
    render(
      <ToastProvider>
        <SettingsPage minChars={8} onDataReplaced={onDataReplaced} />
      </ToastProvider>,
    );
    return { user: userEvent.setup(), onDataReplaced };
  }

  it("lists the backups with their size and time", async () => {
    listBackups.mockResolvedValue([
      backup("Plutus-20261121-093000.db", 2048, "2026-11-21T09:30:00+11:00"),
      backup("Plutus-20261120-200000.db", 512, "2026-11-20T20:00:00+11:00"),
    ]);
    setupBackup();
    const list = await screen.findByRole("list", { name: "Backups" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(2);
    expect(within(list).getByText("Plutus-20261121-093000.db")).toBeInTheDocument();
    expect(within(list).getByText("2026-11-21 09:30 · 2 KB")).toBeInTheDocument();
    expect(within(list).getByText("2026-11-20 20:00 · 512 B")).toBeInTheDocument();
    expect(screen.getByText(/Saved in/)).toHaveTextContent("/plutus/backups");
  });

  it("says so when there are no backups yet", async () => {
    setupBackup();
    expect(await screen.findByText(/No backups yet/)).toBeInTheDocument();
  });

  it("backs up on demand and reloads the list", async () => {
    backupNow.mockResolvedValue(backup("Plutus-20261121-093000.db", 4096));
    const { user } = setupBackup();
    await screen.findByText(/No backups yet/);
    await user.click(screen.getByRole("button", { name: "Back up now" }));
    expect(backupNow).toHaveBeenCalledTimes(1);
    expect(listBackups).toHaveBeenCalledTimes(2);
    expect(await screen.findByText(/Backed up 4 KB/)).toBeInTheDocument();
  });

  it("confirms before restoring and tells the shell to reload", async () => {
    listBackups.mockResolvedValue([backup("Plutus-20261121-093000.db")]);
    restoreBackup.mockResolvedValue(undefined);
    const { user, onDataReplaced } = setupBackup();
    await screen.findByText("Plutus-20261121-093000.db");

    await user.click(screen.getByRole("button", { name: "Restore" }));
    expect(restoreBackup).not.toHaveBeenCalled();
    expect(screen.getByText("Replace all current data?")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Restore" }));
    expect(restoreBackup).toHaveBeenCalledWith("Plutus-20261121-093000.db");
    expect(onDataReplaced).toHaveBeenCalledTimes(1);
    expect(await screen.findByText(/Restored/)).toBeInTheDocument();
  });

  it("can cancel a restore", async () => {
    listBackups.mockResolvedValue([backup("Plutus-20261121-093000.db")]);
    const { user } = setupBackup();
    await screen.findByText("Plutus-20261121-093000.db");
    await user.click(screen.getByRole("button", { name: "Restore" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByText("Replace all current data?")).not.toBeInTheDocument();
    expect(restoreBackup).not.toHaveBeenCalled();
  });

  it("reports a failed restore and keeps the data", async () => {
    listBackups.mockResolvedValue([backup("Plutus-20261121-093000.db")]);
    restoreBackup.mockRejectedValue(new Error("This file is not a Plutus backup."));
    const { user, onDataReplaced } = setupBackup();
    await screen.findByText("Plutus-20261121-093000.db");
    await user.click(screen.getByRole("button", { name: "Restore" }));
    await user.click(screen.getByRole("button", { name: "Restore" }));
    expect(await screen.findByText(/Couldn't restore/)).toBeInTheDocument();
    expect(onDataReplaced).not.toHaveBeenCalled();
  });

  it("shows a load error with a retry", async () => {
    listBackups.mockRejectedValueOnce(new Error("Disk unavailable"));
    const { user } = setupBackup();
    expect(await screen.findByRole("alert")).toHaveTextContent("Disk unavailable");
    listBackups.mockResolvedValue([backup("Plutus-20261121-093000.db")]);
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Plutus-20261121-093000.db")).toBeInTheDocument();
  });
});
