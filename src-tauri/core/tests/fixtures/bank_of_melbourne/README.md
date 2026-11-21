# Bank of Melbourne CSV fixtures

**Synthetic data only. Never put real exports in this repo.**

Format observed from a real Bank of Melbourne export (owner, 2026-11-20;
the real file is not committed):

- File name `transDDMMYY.csv` (e.g. `trans201126.csv` = exported 20 Nov 2026).
- ASCII, CRLF line endings, no BOM.
- Header: `Date,Description,Debit,Credit,Balance`.
- `Date` is **MM/DD/YYYY** (US order): `11/20/2026` = 20 Nov 2026.
- `Debit` = money out, `Credit` = money in, plain positive decimals with no
  `$` or thousands separators; the other column is empty.
- `Balance` = account balance after the row (decimal, no `$`).
- `Description` is space-padded, e.g.
  `Eftpos Debit                  20Nov15:37  Membership    \     Melbourne    Au`
  (type, card date+time `DDMonHH:MM`, merchant, `\`, suburb, country).
- The file has no account identifier, so the user chooses the account.
- Trailing rows of `,,,,` (31 of them in the sample) must be ignored.

Not yet confirmed (one data row in the sample): row order (assumed newest
first), single-digit day/month padding, other description types (Direct
Credit, Osko, fees, transfers), how negative balances appear, and whether
descriptions can contain quoted commas. The fixture's non-Eftpos rows and
the negative balance are guesses.

`synthetic_export.csv` follows the observed format with made-up merchants.
Its balances chain correctly, newest first (each row's balance = the next
row's balance + credit - debit).
