import { forwardRef, ReactNode, SelectHTMLAttributes, useId } from "react";

type Props = Omit<SelectHTMLAttributes<HTMLSelectElement>, "id"> & {
  label: string;
  hint?: string;
  error?: string | null;
  children: ReactNode;
};

/** Labelled native `<select>` with hint/error, matching TextField. */
export const SelectField = forwardRef<HTMLSelectElement, Props>(function SelectField(
  { label, hint, error, className, children, ...rest },
  ref,
) {
  const id = useId();
  const describedBy = error || hint ? `${id}-desc` : undefined;
  return (
    <div className={["field", className].filter(Boolean).join(" ")}>
      <label htmlFor={id}>{label}</label>
      <select
        ref={ref}
        id={id}
        aria-invalid={error ? true : undefined}
        aria-describedby={describedBy}
        {...rest}
      >
        {children}
      </select>
      {(error || hint) && (
        <p id={describedBy} className={error ? "field-error" : "field-hint"}>
          {error || hint}
        </p>
      )}
    </div>
  );
});
