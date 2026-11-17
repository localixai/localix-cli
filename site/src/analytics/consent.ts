// A visitor's answer to the usage statistics card, kept in a first-party cookie on the site's
// registrable domain: one answer covers every site under it (localix.ai and its subdomains), as
// local storage, kept per origin, could not. The cookie only records the answer, which makes it
// strictly necessary; nothing else is stored before a yes.

export type Answer = 'granted' | 'denied';

const NAME = 'analytics_consent';
const YEAR = 60 * 60 * 24 * 365;

/** The answer stored on this device, if any. */
export function storedAnswer(): Answer | undefined {
  const value = cookie(NAME)?.split('.')[0];
  return value === 'granted' || value === 'denied' ? value : undefined;
}

/** Keeps `answer` with the time it was given, for a year, then asks again. */
export function storeAnswer(answer: Answer): void {
  const domain = registrableDomain();
  write(`${NAME}=${answer}.${Math.floor(Date.now() / 1000)}`, YEAR, domain);
}

/** Global Privacy Control: a browser-wide no, taken as this site's while nothing is stored. */
export function privacySignal(): boolean {
  return (navigator as Navigator & { globalPrivacyControl?: boolean }).globalPrivacyControl === true;
}

/** Removes the named cookies wherever a script could have set them: on the host and each of its
 *  parent domains, path `/`. */
export function forgetCookies(names: readonly string[]): void {
  const parts = location.hostname.split('.');
  const domains = [undefined, ...parts.slice(0, -1).map((_, at) => parts.slice(at).join('.'))];
  for (const name of names) for (const domain of domains) write(`${name}=`, 0, domain);
}

/** The names of the cookies this page can read. */
export function cookieNames(): string[] {
  return document.cookie
    .split(';')
    .map((pair) => pair.split('=')[0]?.trim() ?? '')
    .filter(Boolean);
}

function cookie(name: string): string | undefined {
  for (const pair of document.cookie.split(';')) {
    const [key, ...value] = pair.trim().split('=');
    if (key === name) return decodeURIComponent(value.join('='));
  }
  return undefined;
}

function write(pair: string, maxAge: number, domain: string | undefined): void {
  const secure = location.protocol === 'https:' ? '; secure' : '';
  const scope = domain ? `; domain=${domain}` : '';
  document.cookie = `${pair}; path=/; max-age=${maxAge}; samesite=lax${scope}${secure}`;
}

/** The shortest domain of this host a cookie can be set on: `localix.ai` for `docs.localix.ai`,
 *  the host itself under a public suffix (`llx.sh`, `*.pages.dev`), none for `localhost` or an
 *  address. A browser refuses a cookie on a public suffix, so trying from the shortest finds it
 *  without a list. */
function registrableDomain(): string | undefined {
  const parts = location.hostname.split('.');
  if (parts.length < 2 || /^\d+$/.test(parts.at(-1) ?? '') || location.hostname.includes(':')) return undefined;
  const probe = `${NAME}_probe`;
  for (let at = parts.length - 2; at >= 0; at -= 1) {
    const domain = parts.slice(at).join('.');
    write(`${probe}=1`, 10, domain);
    if (cookie(probe) === '1') {
      write(`${probe}=`, 0, domain);
      return domain;
    }
  }
  return undefined;
}
