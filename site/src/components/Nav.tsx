import { GitHubIcon } from './GitHubIcon';
import { GITHUB } from '../links';
import './Nav.css';

export function Nav() {
  return (
    <header className="nav wrap">
      <a className="wordmark" href="#top" aria-label="llx home">
        llx
      </a>
      <a className="gh" href={GITHUB} aria-label="llx on GitHub" title="GitHub">
        <GitHubIcon size={20} />
      </a>
    </header>
  );
}
