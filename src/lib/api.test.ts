import { describe, expect, it } from "vitest";
import { errorMessage, splitError } from "./api";

describe("errorMessage", () => {
  it("capitalises and adds a full stop only when missing", () => {
    expect(errorMessage({ code: "io", message: "i/o error: disk full" })).toBe("I/o error: disk full.");
    expect(errorMessage({ code: "invalid", message: "Name is required." })).toBe("Name is required.");
  });

  it("uses friendly text for known codes and stringifies non-command errors", () => {
    expect(errorMessage({ code: "wrong_password", message: "incorrect password" })).toBe("Incorrect password.");
    expect(errorMessage("boom")).toBe("boom");
  });
});

describe("splitError", () => {
  it("routes field validation errors to the field", () => {
    expect(splitError({ code: "invalid", field: "bsb", message: "BSB must be 6 digits, e.g. 063-000." })).toEqual({
      field: { bsb: "BSB must be 6 digits, e.g. 063-000." },
      general: null,
    });
    expect(splitError({ code: "conflict", field: "name", message: "Taken." }).field).toEqual({ name: "Taken." });
  });

  it("everything else is a general message", () => {
    expect(splitError({ code: "locked", message: "the vault is locked" })).toEqual({
      field: {},
      general: "The vault is locked.",
    });
    expect(splitError({ code: "invalid", message: "no field" }).general).toBe("No field.");
  });
});
