-- 0002: accounts.
--
-- Balances are signed from the owner's point of view: assets positive,
-- liabilities (credit cards, loans) negative, so net worth is a plain sum.
-- Amounts are integer cents (AUD). Dates are 'YYYY-MM-DD'.
CREATE TABLE accounts (
    id                    INTEGER PRIMARY KEY,
    name                  TEXT    NOT NULL CHECK (length(trim(name)) > 0),
    kind                  TEXT    NOT NULL CHECK (kind IN (
                              'transaction', 'savings', 'offset', 'credit_card',
                              'home_loan', 'loan', 'super', 'shares',
                              'investment', 'property', 'cash', 'other')),
    institution           TEXT,
    bsb                   TEXT    CHECK (bsb IS NULL OR (length(bsb) = 6 AND bsb NOT GLOB '*[^0-9]*')),
    account_number        TEXT    CHECK (account_number IS NULL OR account_number NOT GLOB '*[^0-9]*'),
    opening_balance_cents INTEGER NOT NULL DEFAULT 0,
    opening_date          TEXT    NOT NULL,
    -- Offset accounts may point at the loan they offset (several offsets per
    -- loan allowed). Kind/target rules are enforced in Rust.
    offset_for_account_id INTEGER REFERENCES accounts (id) ON DELETE RESTRICT,
    notes                 TEXT,
    archived_at           TEXT,
    created_at            TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    CHECK (offset_for_account_id IS NULL OR kind = 'offset'),
    CHECK (offset_for_account_id IS NULL OR offset_for_account_id <> id)
) STRICT;

-- Active account names are unique (case-insensitive); archived ones free the name.
CREATE UNIQUE INDEX accounts_active_name ON accounts (name COLLATE NOCASE)
    WHERE archived_at IS NULL;

CREATE INDEX accounts_offset_for ON accounts (offset_for_account_id)
    WHERE offset_for_account_id IS NOT NULL;
