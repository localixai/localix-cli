/** Reading the bank's CSV export. */
export type Transaction = {
  date: string; // YYYY-MM-DD
  category: string;
  amount: number;
  description: string;
};

export function parseLine(line: string): Transaction {
  const [date = '', category = '', amount = '', description = ''] = line.trim().split(',');
  return { date, category: category.trim(), amount: Number(amount), description };
}

export function parseCsv(text: string): Transaction[] {
  return text
    .split('\n')
    .slice(1)
    .filter((line) => line.trim() !== '')
    .map(parseLine);
}
