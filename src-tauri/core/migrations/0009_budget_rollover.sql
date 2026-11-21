-- 0009: optional budget rollover per category (off by default).
--
-- With rollover on, a month's unspent budget is added to the next month's
-- (and overspending is taken off it), for as long as the category stays
-- budgeted.
ALTER TABLE categories ADD COLUMN budget_rollover INTEGER NOT NULL DEFAULT 0
    CHECK (budget_rollover IN (0, 1));
