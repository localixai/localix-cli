import { CI_DOCS } from '../links';
import './Ci.css';

const CODES: [string, string, string, string][] = [
  ['0', 'achieved', 'ok', 'the goal was reached'],
  ['1', 'not_achieved', 'bad', 'it was not, or the data missed the schema; blocked_by says why'],
  ['2', 'exhausted', '', 'out of turns, time or money'],
  ['3', 'error', '', 'no key, no network, a bad argument'],
  ['130', 'interrupted', '', 'Ctrl-C or SIGTERM'],
];

/** The contract in one place: the command a job runs, what it gets back, and what each exit code means. */
export function Ci() {
  return (
    <section className="ci wrap" aria-labelledby="ci-title">
      <div className="ci-head">
        <h2 id="ci-title">A task your pipeline can branch on</h2>
        <p>
          <code>llx exec</code> runs one task and ends with a <code>result</code>: achieved or not, a summary, and data
          matching your JSON Schema. Text alone never ends a task.
        </p>
      </div>
      <div className="ci-body">
        <pre className="ci-panel" aria-label="A CI step: the command, the result and the exit code">
          <span className="t-dim"># what a CI job runs</span>
          {'\n'}
          <span className="t-dim">$ </span>
          {'llx exec --json --mode plan --schema @ship.json \\\n    "Safe to ship this diff? Which version bump?" > events.jsonl\n'}
          <span className="t-dim">$ </span>
          {'echo $?\n'}
          <span className="t-ok">0{'\n'}</span>
          <span className="t-dim">$ </span>
          {"jq -c 'select(.type == \"run.completed\") | {status, data, cost: .usage.cost_usd}' events.jsonl\n"}
          <span className="t-hi">{'{"status":"achieved","data":{"bump":"patch","ship":true},"cost":0.004}'}</span>
        </pre>
        <table className="ci-codes">
          <caption>Exit code, and the same status in the last JSON event</caption>
          <tbody>
            {CODES.map(([code, status, tone, text]) => (
              <tr key={code}>
                <th scope="row" className={tone ? `t-${tone}` : undefined}>
                  {code}
                </th>
                <td>
                  <code>{status}</code>
                </td>
                <td>{text}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="ci-more">
        On GitHub, <code>uses: rsaryev/llx@v0</code> runs the same step and returns the answer, the status and the spend.{' '}
        <a href={CI_DOCS}>The CI guide</a>
      </p>
    </section>
  );
}
