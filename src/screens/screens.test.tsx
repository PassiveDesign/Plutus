import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../lib/api";
import { SetupScreen } from "./SetupScreen";
import { UnlockScreen } from "./UnlockScreen";

vi.mock("../lib/api", async (orig) => {
  const real = await orig<typeof import("../lib/api")>();
  return {
    ...real,
    api: { ...real.api, createVault: vi.fn(), unlockVault: vi.fn() },
  };
});

const createVault = vi.mocked(api.createVault);
const unlockVault = vi.mocked(api.unlockVault);

beforeEach(() => {
  createVault.mockReset();
  unlockVault.mockReset();
});

describe("SetupScreen", () => {
  async function fill(pw: string, confirm: string, tick: boolean) {
    const user = userEvent.setup();
    await user.type(screen.getByLabelText("Password"), pw);
    await user.type(screen.getByLabelText("Confirm password"), confirm);
    if (tick) await user.click(screen.getByRole("checkbox"));
    return user;
  }

  it("requires min length, matching confirm and the no-recovery acknowledgement", async () => {
    render(<SetupScreen minChars={8} onCreated={() => {}} />);
    const submit = screen.getByRole("button", { name: "Create password" });

    const user = await fill("short", "short", true);
    expect(screen.getByLabelText("Password")).toHaveAttribute("aria-invalid", "true");
    expect(submit).toBeDisabled();

    await user.clear(screen.getByLabelText("Password"));
    await user.type(screen.getByLabelText("Password"), "long enough");
    expect(screen.getByLabelText("Confirm password")).toHaveAccessibleDescription(
      "Passwords don't match.",
    );
    expect(submit).toBeDisabled();

    await user.clear(screen.getByLabelText("Confirm password"));
    await user.type(screen.getByLabelText("Confirm password"), "long enough");
    expect(submit).toBeEnabled();

    await user.click(screen.getByRole("checkbox")); // untick
    expect(submit).toBeDisabled();
  });

  it("counts characters, not UTF-16 units", async () => {
    render(<SetupScreen minChars={8} onCreated={() => {}} />);
    await fill("🔐🔐🔐🔐🔐🔐🔐", "🔐🔐🔐🔐🔐🔐🔐", true);
    expect(screen.getByRole("button", { name: "Create password" })).toBeDisabled();
  });

  it("creates the vault and reports success", async () => {
    createVault.mockResolvedValue(undefined);
    const onCreated = vi.fn();
    render(<SetupScreen minChars={8} onCreated={onCreated} />);
    const user = await fill("long enough", "long enough", true);
    await user.click(screen.getByRole("button", { name: "Create password" }));
    expect(createVault).toHaveBeenCalledWith("long enough");
    expect(onCreated).toHaveBeenCalledOnce();
  });

  it("shows backend errors and stays on the screen", async () => {
    createVault.mockRejectedValue({ code: "io", message: "i/o error: disk full" });
    const onCreated = vi.fn();
    render(<SetupScreen minChars={8} onCreated={onCreated} />);
    const user = await fill("long enough", "long enough", true);
    await user.click(screen.getByRole("button", { name: "Create password" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("I/o error: disk full.");
    expect(onCreated).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Create password" })).toBeEnabled();
  });
});

describe("UnlockScreen", () => {
  it("wrong password shows an error, clears and refocuses the field", async () => {
    unlockVault.mockRejectedValue({ code: "wrong_password", message: "incorrect password" });
    const onUnlocked = vi.fn();
    const user = userEvent.setup();
    render(<UnlockScreen onUnlocked={onUnlocked} />);
    const field = screen.getByLabelText("Password");
    await user.type(field, "nope{Enter}");

    expect(await screen.findByText("Incorrect password.")).toBeInTheDocument();
    expect(field).toHaveValue("");
    await vi.waitFor(() => expect(field).toHaveFocus());
    expect(onUnlocked).not.toHaveBeenCalled();
  });

  it("returns focus to the field after a failed attempt (browsers blur disabled inputs)", async () => {
    let reject!: (e: unknown) => void;
    unlockVault.mockImplementation(() => new Promise((_, r) => (reject = r)));
    const user = userEvent.setup();
    render(<UnlockScreen onUnlocked={() => {}} />);
    const field = screen.getByLabelText("Password");
    await user.type(field, "nope{Enter}");

    // While the attempt is pending the field is disabled; real browsers drop
    // focus from it. jsdom doesn't, so simulate that.
    expect(field).toBeDisabled();
    field.blur();
    expect(field).not.toHaveFocus();

    reject({ code: "wrong_password", message: "incorrect password" });
    expect(await screen.findByText("Incorrect password.")).toBeInTheDocument();
    await vi.waitFor(() => expect(field).toHaveFocus());
  });

  it("correct password unlocks", async () => {
    unlockVault.mockResolvedValue(undefined);
    const onUnlocked = vi.fn();
    const user = userEvent.setup();
    render(<UnlockScreen onUnlocked={onUnlocked} />);
    await user.type(screen.getByLabelText("Password"), "right password");
    await user.click(screen.getByRole("button", { name: "Unlock" }));
    expect(unlockVault).toHaveBeenCalledWith("right password");
    expect(onUnlocked).toHaveBeenCalledOnce();
  });

  it("submit is disabled while the field is empty", () => {
    render(<UnlockScreen onUnlocked={() => {}} />);
    expect(screen.getByRole("button", { name: "Unlock" })).toBeDisabled();
  });
});
