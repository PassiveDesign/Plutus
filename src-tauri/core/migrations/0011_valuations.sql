-- 0011: valuations for accounts tracked by market value (super, shares,
-- investments, property). One valuation per account per day; re-entering the
-- same date replaces it. Values are signed cents (AUD) from the owner's point
-- of view: assets positive.
CREATE TABLE valuations (
    id          INTEGER PRIMARY KEY,
    account_id  INTEGER NOT NULL REFERENCES accounts (id) ON DELETE RESTRICT,
    date        TEXT    NOT NULL CHECK (date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    value_cents INTEGER NOT NULL,
    note        TEXT,
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE (account_id, date)
) STRICT;

CREATE INDEX valuations_account_date ON valuations (account_id, date DESC);
