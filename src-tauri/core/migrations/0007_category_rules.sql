-- 0007: auto-categorisation rules.
--
-- A rule matches when `pattern` appears (case-insensitive) in the payee,
-- the bank description, or either, optionally only for money in or out.
-- Rules are tried in sort_order; the first match sets the category. Rules
-- never overwrite a category that is already set, and skip transfers.
CREATE TABLE category_rules (
    id          INTEGER PRIMARY KEY,
    pattern     TEXT    NOT NULL CHECK (length(trim(pattern)) > 0),
    match_field TEXT    NOT NULL DEFAULT 'either'
                        CHECK (match_field IN ('payee', 'description', 'either')),
    direction   TEXT    CHECK (direction IS NULL OR direction IN ('in', 'out')),
    category_id INTEGER NOT NULL REFERENCES categories (id) ON DELETE RESTRICT,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
) STRICT;
