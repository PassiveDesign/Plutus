import { ReactNode } from "react";
import { formatCents } from "../lib/money";

export type Column<T> = {
  key: string;
  header: string;
  /** Cell content. For money columns use `money` instead. */
  render?: (row: T) => ReactNode;
  /** Integer cents; rendered with `formatCents`, right-aligned, red if negative. */
  money?: (row: T) => number;
  align?: "left" | "right";
  width?: string;
};

type Props<T> = {
  columns: Column<T>[];
  rows: T[];
  rowKey: (row: T) => string | number;
  empty?: ReactNode;
  onRowClick?: (row: T) => void;
  caption?: string;
};

export function DataTable<T>({ columns, rows, rowKey, empty, onRowClick, caption }: Props<T>) {
  if (rows.length === 0) {
    return <div className="table-empty">{empty ?? "Nothing here yet."}</div>;
  }
  return (
    <div className="table-wrap">
      <table className="data-table">
        {caption && <caption className="sr-only">{caption}</caption>}
        <thead>
          <tr>
            {columns.map((c) => (
              <th
                key={c.key}
                scope="col"
                style={{ width: c.width }}
                className={c.money || c.align === "right" ? "num" : undefined}
              >
                {c.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr
              key={rowKey(row)}
              className={onRowClick ? "clickable" : undefined}
              onClick={onRowClick ? () => onRowClick(row) : undefined}
              tabIndex={onRowClick ? 0 : undefined}
              onKeyDown={
                onRowClick
                  ? (e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        onRowClick(row);
                      }
                    }
                  : undefined
              }
            >
              {columns.map((c) => {
                if (c.money) {
                  const cents = c.money(row);
                  return (
                    <td key={c.key} className={`num money${cents < 0 ? " negative" : ""}`}>
                      {formatCents(cents)}
                    </td>
                  );
                }
                return (
                  <td key={c.key} className={c.align === "right" ? "num" : undefined}>
                    {c.render ? c.render(row) : null}
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
