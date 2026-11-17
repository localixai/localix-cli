import { OPEN } from '../analytics/card';
import { measurementId } from '../analytics';
import { DOCS, GITHUB, MAKER } from '../links';
import './Footer.css';

/** Under every page, one strip: who makes llx, its one line, then where else to go. With usage
 *  statistics on, the last link opens their card again, so a visitor can change their answer from
 *  any page. */
export function Footer() {
  return (
    <footer className="foot wrap">
      <span>
        <a className="foot-maker" href={MAKER.href}>
          {MAKER.label}
        </a>
        llx is MIT-licensed and open source.
      </span>
      <nav aria-label="Footer">
        <a href={DOCS}>Docs</a>
        <a href={GITHUB}>GitHub</a>
        <a href={`${GITHUB}/blob/main/MODELS.md`}>Models</a>
        <a href={`${GITHUB}/blob/main/CHANGELOG.md`}>Changelog</a>
        {measurementId() && (
          <button type="button" className="foot-consent" {...{ [OPEN]: '' }}>
            Cookie settings
          </button>
        )}
      </nav>
    </footer>
  );
}
