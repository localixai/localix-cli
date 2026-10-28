import type { Transaction } from '../lib/csv';
import { money, report } from '../lib/report';

/** Each month's spending per category, oldest month first. */
export function MonthReport({ transactions }: { transactions: Transaction[] }) {
  return (
    <div className="report">
      {report(transactions).map((m) => (
        <section key={m.month}>
          <h2>{m.month}</h2>
          <table>
            <tbody>
              {m.categories.map(([name, amount]) => (
                <tr key={name}>
                  <td>{name}</td>
                  <td>{money(amount)}</td>
                </tr>
              ))}
              <tr className="total">
                <td>total</td>
                <td>{money(m.total)}</td>
              </tr>
            </tbody>
          </table>
        </section>
      ))}
    </div>
  );
}
