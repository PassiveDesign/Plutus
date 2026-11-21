-- 0006: import de-duplication.
--
-- Imported rows carry a key built from the bank's own data (date, amount,
-- description and an occurrence number for identical rows in one file).
-- Re-importing an overlapping export produces the same keys, so rows that
-- are already in the account are recognised. Hand-entered transactions have
-- NULL.
ALTER TABLE transactions ADD COLUMN import_key TEXT;

CREATE UNIQUE INDEX transactions_import_key ON transactions (account_id, import_key)
    WHERE import_key IS NOT NULL;
