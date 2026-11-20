-- 0003: transactions.
--
-- amount_cents is signed from the account's point of view: money in
-- positive, money out negative. Dates are 'YYYY-MM-DD'.
-- Later milestones add columns with ALTER TABLE ... ADD COLUMN (nullable):
-- category_id (M3), transfer_id (M2 transfers), import_hash (M4).
CREATE TABLE transactions (
    id           INTEGER PRIMARY KEY,
    account_id   INTEGER NOT NULL REFERENCES accounts (id) ON DELETE RESTRICT,
    date         TEXT    NOT NULL CHECK (date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    amount_cents INTEGER NOT NULL,
    payee        TEXT,
    -- Raw bank description, kept verbatim for import matching.
    description  TEXT,
    memo         TEXT,
    cleared      INTEGER NOT NULL DEFAULT 0 CHECK (cleared IN (0, 1)),
    created_at   TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at   TEXT,
    CHECK (payee IS NOT NULL OR description IS NOT NULL)
) STRICT;

CREATE INDEX transactions_account_date ON transactions (account_id, date, id);
CREATE INDEX transactions_date ON transactions (date, id);
