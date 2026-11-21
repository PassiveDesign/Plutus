-- 0004: transfers between accounts.
--
-- A transfer is two transactions (money out of one account, the same amount
-- into another, same date) sharing a transfer_id. transfer_id is the id of
-- the outgoing leg. Ordinary transactions have NULL.
ALTER TABLE transactions ADD COLUMN transfer_id INTEGER;

CREATE INDEX transactions_transfer ON transactions (transfer_id)
    WHERE transfer_id IS NOT NULL;
