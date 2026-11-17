// The only file that names Google's tag, its queue and its host (prerender.mjs fails the build
// otherwise). Basic consent mode: none of this runs before a visitor's yes, so before it the page
// has no queue, no script, no cookie and sends nothing. The advertising signals are denied in every
// command, after a yes too: the site runs no advertising.

declare global {
  interface Window {
    dataLayer?: unknown[];
    [disable: `ga-disable-${string}`]: boolean | undefined;
  }
}

const NO_ADS = { ad_storage: 'denied', ad_user_data: 'denied', ad_personalization: 'denied' } as const;
const HOST = 'https://www.googletagmanager.com';

let loaded: string | undefined;

/** Queues a command the way the tag reads it: as a call's `arguments`, not as an array. */
function gtag(..._command: unknown[]): void {
  // The tag tells a command from data by its being an Arguments object.
  (window.dataLayer ??= []).push(arguments);
}

/** Starts measuring after a yes: the consent, then the tag's settings, then the script, which
 *  sends the page's view. A second yes after a withdrawal turns it back on without a second script. */
export function startTag(id: string): void {
  window[`ga-disable-${id}`] = false;
  if (loaded) {
    gtag('consent', 'update', { ...NO_ADS, analytics_storage: 'granted' });
    return;
  }
  loaded = id;
  gtag('consent', 'default', { ...NO_ADS, analytics_storage: 'granted' });
  gtag('js', new Date());
  gtag('config', id, { allow_google_signals: false, allow_ad_personalization_signals: false });
  const script = document.createElement('script');
  script.async = true;
  script.src = `${HOST}/gtag/js?id=${encodeURIComponent(id)}`;
  document.head.append(script);
}

/** Stops measuring at once: storage denied, and Google's own off switch, since a loaded tag would
 *  still send its automatic hits without storage. */
export function stopTag(id: string): void {
  window[`ga-disable-${id}`] = true;
  if (loaded) gtag('consent', 'update', { ...NO_ADS, analytics_storage: 'denied' });
}

/** One event, while measuring; nothing otherwise. */
export function sendEvent(name: string, params: Record<string, string>): void {
  if (!loaded || window[`ga-disable-${loaded}`]) return;
  gtag('event', name, params);
}

/** The cookies the tag sets for a measurement id: `_ga` and `_ga_<id without G->`. */
export function tagCookies(id: string, names: readonly string[]): string[] {
  const own = `_ga_${id.replace(/^G-/, '')}`;
  return names.filter((name) => name === '_ga' || name === own);
}
