import { forwardRef, InputHTMLAttributes, useId } from "react";

type Props = Omit<InputHTMLAttributes<HTMLInputElement>, "id"> & {
  label: string;
  /** Helper text under the field. Replaced by `error` when present. */
  hint?: string;
  error?: string | null;
};

/** Labelled input with hint and error text, wired up for screen readers. */
export const TextField = forwardRef<HTMLInputElement, Props>(function TextField(
  { label, hint, error, className, type = "text", ...rest },
  ref,
) {
  const id = useId();
  const describedBy = error || hint ? `${id}-desc` : undefined;
  return (
    <div className={["field", className].filter(Boolean).join(" ")}>
      <label htmlFor={id}>{label}</label>
      <input
        ref={ref}
        id={id}
        type={type}
        aria-invalid={error ? true : undefined}
        aria-describedby={describedBy}
        {...rest}
      />
      {(error || hint) && (
        <p id={describedBy} className={error ? "field-error" : "field-hint"}>
          {error || hint}
        </p>
      )}
    </div>
  );
});
