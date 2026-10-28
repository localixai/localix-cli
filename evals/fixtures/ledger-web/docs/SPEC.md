# Ledger: the monthly report

The app reads the bank's CSV export and shows what was spent per category in each month.

## Input

A CSV text with a header line `date,category,amount,description`.

- `date` is `YYYY-MM-DD`, or `DD.MM.YYYY`: exports from the bank's older accounts write it that way.
- `amount` is a decimal number with at most two decimals. A purchase is positive; a refund is
  negative.
- `description` is free text. It may hold commas, and is then quoted as CSV quotes it (`"Taxi,
  airport"`; a quote inside is doubled).

## The report

- One block per month, oldest first; in a month, categories sorted by name, then the month's total.
- A refund counts against its own category: a category's amount is its purchases minus its refunds.
- Money is exact: amounts are summed as integer cents, never as floating-point numbers, and shown
  with two decimals (`30.30`, `-5.00`).
- `<MonthReport>` takes an optional `month` (`YYYY-MM`): given one, it shows that month alone. A month
  with no transactions shows its heading and a total of `0.00`.
- `<App>` has a month picker (a `<select>` labelled "Month") whose options are "All months", then
  every month in the data, oldest first; choosing one passes it to `<MonthReport>`.
