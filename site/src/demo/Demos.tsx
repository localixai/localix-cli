import { useState } from 'react';
import { Demo } from './Demo';
import { Film } from './Film';
import './Demos.css';

const TABS = [
  { id: 'terminal', name: 'In the terminal', note: 'Providers, models, modes and a task, recorded' },
  { id: 'window', name: 'In the window', note: 'It opens the page, tests it, fixes what it finds' },
] as const;

/** The two demos, one at a time: the real session first, then the window it works in. */
export function Demos() {
  const [shown, setShown] = useState<(typeof TABS)[number]['id']>('terminal');
  return (
    <div className="demos hero-demo">
      <div className="demos-tabs" role="tablist" aria-label="Demos">
        {TABS.map((tab, i) => (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={shown === tab.id}
            className="demos-tab"
            onClick={() => setShown(tab.id)}
          >
            <span className="demos-n">{i + 1}</span>
            <span>
              <b>{tab.name}</b>
              <small>{tab.note}</small>
            </span>
          </button>
        ))}
      </div>
      <div role="tabpanel">
        {shown === 'terminal' ? (
          <Film src="/demo-llx.cast" label="A real llx session, recorded with asciinema" />
        ) : (
          <Demo />
        )}
      </div>
    </div>
  );
}
