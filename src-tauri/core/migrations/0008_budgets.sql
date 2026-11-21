-- 0008: monthly budgets for expense categories.
--
-- A row sets a category's monthly budget from `month` onwards, until a
-- later row changes it (so a budget is entered once, not every month).
-- Amounts are positive cents; 0 stops budgeting the category.
CREATE TABLE budgets (
    category_id  INTEGER NOT NULL REFERENCES categories (id) ON DELETE RESTRICT,
    month        TEXT    NOT NULL CHECK (month GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]'),
    amount_cents INTEGER NOT NULL CHECK (amount_cents >= 0),
    PRIMARY KEY (category_id, month)
) STRICT, WITHOUT ROWID;
