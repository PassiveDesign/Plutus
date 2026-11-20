import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../components";
import { api } from "../lib/api";
import { SettingsPage } from "./SettingsPage";

vi.mock("../lib/api", async (orig) => {
  const real = await orig<typeof import("../lib/api")>();
  return { ...real, api: { ...real.api, changePassword: vi.fn() } };
});
const changePassword = vi.mocked(api.changePassword);

beforeEach(() => {
  changePassword.mockReset();
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
