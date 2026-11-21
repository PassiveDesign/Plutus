-- 0005: categories (two levels: groups contain categories).
--
-- A group is either 'income' or 'expense' (budgets and reports use this).
-- Categories and groups are archived, never deleted, so history keeps its
-- labels. Transfers are not categorised.
CREATE TABLE category_groups (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL CHECK (length(trim(name)) > 0),
    kind        TEXT    NOT NULL CHECK (kind IN ('income', 'expense')),
    sort_order  INTEGER NOT NULL DEFAULT 0,
    archived_at TEXT,
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
) STRICT;

CREATE UNIQUE INDEX category_groups_active_name ON category_groups (name COLLATE NOCASE)
    WHERE archived_at IS NULL;

CREATE TABLE categories (
    id          INTEGER PRIMARY KEY,
    group_id    INTEGER NOT NULL REFERENCES category_groups (id) ON DELETE RESTRICT,
    name        TEXT    NOT NULL CHECK (length(trim(name)) > 0),
    sort_order  INTEGER NOT NULL DEFAULT 0,
    archived_at TEXT,
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
) STRICT;

-- Active names are unique within a group (case-insensitive).
CREATE UNIQUE INDEX categories_active_name ON categories (group_id, name COLLATE NOCASE)
    WHERE archived_at IS NULL;

ALTER TABLE transactions ADD COLUMN category_id INTEGER
    REFERENCES categories (id) ON DELETE RESTRICT;

CREATE INDEX transactions_category ON transactions (category_id)
    WHERE category_id IS NOT NULL;

-- Defaults for an Australian household; rename/archive freely.
INSERT INTO category_groups (name, kind, sort_order) VALUES
    ('Income',            'income',  1),
    ('Home',              'expense', 2),
    ('Utilities',         'expense', 3),
    ('Food',              'expense', 4),
    ('Transport',         'expense', 5),
    ('Health',            'expense', 6),
    ('Personal',          'expense', 7),
    ('Lifestyle',         'expense', 8),
    ('Family & pets',     'expense', 9),
    ('Financial',         'expense', 10),
    ('Other',             'expense', 11);

INSERT INTO categories (group_id, name, sort_order)
SELECT g.id, c.name, c.ord
FROM (
              SELECT 'Income' AS grp, 'Salary & wages' AS name, 1 AS ord
    UNION ALL SELECT 'Income', 'Interest', 2
    UNION ALL SELECT 'Income', 'Government payments', 3
    UNION ALL SELECT 'Income', 'Refunds & reimbursements', 4
    UNION ALL SELECT 'Income', 'Other income', 5
    UNION ALL SELECT 'Home', 'Rent', 1
    UNION ALL SELECT 'Home', 'Council rates', 2
    UNION ALL SELECT 'Home', 'Strata / body corporate', 3
    UNION ALL SELECT 'Home', 'Home & contents insurance', 4
    UNION ALL SELECT 'Home', 'Repairs & maintenance', 5
    UNION ALL SELECT 'Home', 'Furniture & household', 6
    UNION ALL SELECT 'Utilities', 'Electricity', 1
    UNION ALL SELECT 'Utilities', 'Gas', 2
    UNION ALL SELECT 'Utilities', 'Water', 3
    UNION ALL SELECT 'Utilities', 'Internet', 4
    UNION ALL SELECT 'Utilities', 'Mobile phone', 5
    UNION ALL SELECT 'Food', 'Groceries', 1
    UNION ALL SELECT 'Food', 'Eating out', 2
    UNION ALL SELECT 'Food', 'Takeaway & coffee', 3
    UNION ALL SELECT 'Transport', 'Fuel', 1
    UNION ALL SELECT 'Transport', 'Rego', 2
    UNION ALL SELECT 'Transport', 'Car insurance', 3
    UNION ALL SELECT 'Transport', 'Servicing & repairs', 4
    UNION ALL SELECT 'Transport', 'Public transport', 5
    UNION ALL SELECT 'Transport', 'Tolls & parking', 6
    UNION ALL SELECT 'Health', 'Medical', 1
    UNION ALL SELECT 'Health', 'Pharmacy', 2
    UNION ALL SELECT 'Health', 'Dental & optical', 3
    UNION ALL SELECT 'Health', 'Health insurance', 4
    UNION ALL SELECT 'Health', 'Fitness', 5
    UNION ALL SELECT 'Personal', 'Clothing', 1
    UNION ALL SELECT 'Personal', 'Personal care', 2
    UNION ALL SELECT 'Personal', 'Gifts & donations', 3
    UNION ALL SELECT 'Personal', 'Education', 4
    UNION ALL SELECT 'Lifestyle', 'Subscriptions', 1
    UNION ALL SELECT 'Lifestyle', 'Entertainment', 2
    UNION ALL SELECT 'Lifestyle', 'Hobbies', 3
    UNION ALL SELECT 'Lifestyle', 'Holidays & travel', 4
    UNION ALL SELECT 'Family & pets', 'Childcare & school', 1
    UNION ALL SELECT 'Family & pets', 'Kids', 2
    UNION ALL SELECT 'Family & pets', 'Pets', 3
    UNION ALL SELECT 'Financial', 'Bank fees', 1
    UNION ALL SELECT 'Financial', 'Interest charged', 2
    UNION ALL SELECT 'Financial', 'Life & income insurance', 3
    UNION ALL SELECT 'Financial', 'Tax', 4
    UNION ALL SELECT 'Other', 'Cash withdrawals', 1
    UNION ALL SELECT 'Other', 'Miscellaneous', 2
) c
JOIN category_groups g ON g.name = c.grp;
