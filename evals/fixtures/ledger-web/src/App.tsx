import { parseCsv } from './lib/csv';
import { MonthReport } from './components/MonthReport';
import { SAMPLE } from './sample';

export function App({ csv = SAMPLE }: { csv?: string }) {
  const transactions = parseCsv(csv);
  return (
    <main>
      <h1>Ledger</h1>
      <MonthReport transactions={transactions} />
    </main>
  );
}
