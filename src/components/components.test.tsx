import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { Button, DataTable, Modal, TextField, ToastProvider, useToast } from "./index";
import { TOAST_MS } from "./Toast";

describe("Button", () => {
  it("defaults to type=button and primary variant", () => {
    render(<Button>Save</Button>);
    const b = screen.getByRole("button", { name: "Save" });
    expect(b).toHaveAttribute("type", "button");
    expect(b).toHaveClass("btn", "btn-primary");
  });

  it("busy disables and swaps the label", async () => {
    const onClick = vi.fn();
    render(
      <Button busy busyLabel="Saving…" onClick={onClick}>
        Save
      </Button>,
    );
    const b = screen.getByRole("button", { name: "Saving…" });
    expect(b).toBeDisabled();
    expect(b).toHaveAttribute("aria-busy", "true");
    await userEvent.click(b);
    expect(onClick).not.toHaveBeenCalled();
  });
});

describe("TextField", () => {
  it("links label, input and hint", () => {
    render(<TextField label="Name" hint="As shown on statements" />);
    const input = screen.getByLabelText("Name");
    expect(input).toHaveAccessibleDescription("As shown on statements");
    expect(input).not.toHaveAttribute("aria-invalid");
  });

  it("error replaces hint and marks the input invalid", () => {
    render(<TextField label="Name" hint="hint" error="Required" />);
    const input = screen.getByLabelText("Name");
    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(input).toHaveAccessibleDescription("Required");
    expect(screen.queryByText("hint")).toBeNull();
  });
});

function ModalHarness({ onClose = () => {} }: { onClose?: () => void }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button onClick={() => setOpen(true)}>Open</button>
      <Modal
        open={open}
        title="Edit account"
        onClose={() => {
          onClose();
          setOpen(false);
        }}
        footer={<Button>Save</Button>}
      >
        <TextField label="Account name" />
      </Modal>
    </>
  );
}

describe("Modal", () => {
  it("renders nothing when closed", () => {
    render(<Modal open={false} title="x" onClose={() => {}}>body</Modal>);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("opens labelled, focuses inside, Escape closes and restores focus", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<ModalHarness onClose={onClose} />);
    const opener = screen.getByRole("button", { name: "Open" });
    await user.click(opener);

    const dialog = screen.getByRole("dialog", { name: "Edit account" });
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(dialog.contains(document.activeElement)).toBe(true);

    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledOnce();
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(opener).toHaveFocus();
  });

  it("traps Tab focus inside the dialog", async () => {
    const user = userEvent.setup();
    render(<ModalHarness />);
    await user.click(screen.getByRole("button", { name: "Open" }));
    const dialog = screen.getByRole("dialog");
    for (let i = 0; i < 6; i++) {
      await user.tab();
      expect(dialog.contains(document.activeElement)).toBe(true);
    }
    for (let i = 0; i < 6; i++) {
      await user.tab({ shift: true });
      expect(dialog.contains(document.activeElement)).toBe(true);
    }
  });

  it("closes on backdrop click and the close button, not on panel click", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<ModalHarness onClose={onClose} />);
    await user.click(screen.getByRole("button", { name: "Open" }));
    await user.click(screen.getByRole("dialog"));
    expect(onClose).not.toHaveBeenCalled();
    await user.click(document.querySelector(".modal-backdrop")!);
    expect(onClose).toHaveBeenCalledOnce();

    await user.click(screen.getByRole("button", { name: "Open" }));
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalledTimes(2);
  });
});

type Row = { id: number; name: string; balance: number };
const rows: Row[] = [
  { id: 1, name: "Everyday", balance: 123_456 },
  { id: 2, name: "Credit card", balance: -4_250 },
];

describe("DataTable", () => {
  const columns = [
    { key: "name", header: "Account", render: (r: Row) => r.name },
    { key: "balance", header: "Balance", money: (r: Row) => r.balance },
  ];

  it("shows the empty state when there are no rows", () => {
    render(<DataTable columns={columns} rows={[]} rowKey={(r) => r.id} empty="No accounts yet." />);
    expect(screen.getByText("No accounts yet.")).toBeInTheDocument();
    expect(screen.queryByRole("table")).toBeNull();
  });

  it("formats money columns, right-aligns them and flags negatives", () => {
    render(<DataTable columns={columns} rows={rows} rowKey={(r) => r.id} />);
    const [, first, second] = screen.getAllByRole("row");
    const pos = within(first).getByText("$1,234.56");
    const neg = within(second).getByText("-$42.50");
    expect(pos).toHaveClass("num", "money");
    expect(pos).not.toHaveClass("negative");
    expect(neg).toHaveClass("negative");
    expect(screen.getByRole("columnheader", { name: "Balance" })).toHaveClass("num");
  });

  it("row click and Enter both activate", async () => {
    const user = userEvent.setup();
    const onRowClick = vi.fn();
    render(<DataTable columns={columns} rows={rows} rowKey={(r) => r.id} onRowClick={onRowClick} />);
    await user.click(screen.getByText("Everyday"));
    expect(onRowClick).toHaveBeenLastCalledWith(rows[0]);
    screen.getByText("Credit card").closest("tr")!.focus();
    await user.keyboard("{Enter}");
    expect(onRowClick).toHaveBeenLastCalledWith(rows[1]);
  });
});

function ToastButtons() {
  const toast = useToast();
  return (
    <>
      <button onClick={() => toast.error("Save failed")}>err</button>
      <button onClick={() => toast.success("Saved")}>ok</button>
    </>
  );
}

describe("Toast", () => {
  it("errors persist until dismissed; success auto-dismisses", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
      render(
        <ToastProvider>
          <ToastButtons />
        </ToastProvider>,
      );
      await user.click(screen.getByRole("button", { name: "err" }));
      await user.click(screen.getByRole("button", { name: "ok" }));
      expect(screen.getByRole("alert")).toHaveTextContent("Save failed");
      expect(screen.getByRole("status")).toHaveTextContent("Saved");

      act(() => vi.advanceTimersByTime(TOAST_MS + 10));
      expect(screen.queryByRole("status")).toBeNull();
      expect(screen.getByRole("alert")).toBeInTheDocument();

      await user.click(screen.getByRole("button", { name: "Dismiss" }));
      expect(screen.queryByRole("alert")).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("useToast outside the provider throws a clear error", () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    expect(() => render(<ToastButtons />)).toThrow(/ToastProvider/);
  });
});
