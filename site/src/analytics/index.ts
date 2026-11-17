// Usage statistics: Google Analytics 4 behind a visitor's yes. Off while `ANALYTICS` in links.ts
// is undefined: then every call here does nothing.
import { ANALYTICS } from '../links';
import { cookieNames, forgetCookies, storeAnswer, storedAnswer, type Answer } from './consent';
import { type AnalyticsEvent, type AnalyticsEvents, wireParams } from './events';
import { sendEvent, startTag, stopTag, tagCookies } from './google-tag';

export type { Answer } from './consent';
export { privacySignal, storedAnswer } from './consent';

const ID = /^G-[A-Z0-9]{4,20}$/;

/** The site's measurement id, or undefined when statistics are off. A malformed one fails the
 *  build (the footer asks for it while the pages are rendered). */
export function measurementId(): string | undefined {
  const id = ANALYTICS?.ga4;
  if (id === undefined) return undefined;
  if (!ID.test(id)) throw new Error(`ANALYTICS.ga4 in src/links.ts is not a GA4 measurement id: "${id}"`);
  return id;
}

/** Keeps the visitor's answer and acts on it: a yes starts measuring, a no stops it and removes
 *  Google Analytics' cookies. */
export function answer(given: Answer): void {
  const id = measurementId();
  if (!id) return;
  storeAnswer(given);
  if (given === 'granted') {
    startTag(id);
    return;
  }
  stopTag(id);
  forgetCookies(tagCookies(id, cookieNames()));
}

/** Starts measuring at a page's load when this device said yes before. */
export function resumeAnalytics(): void {
  const id = measurementId();
  if (id && storedAnswer() === 'granted') startTag(id);
}

/** Sends one event, when measuring; a parameter outside its set drops it. */
export function track<E extends AnalyticsEvent>(name: E, params: AnalyticsEvents[E]): void {
  const wire = wireParams(params);
  if (wire) sendEvent(name, wire);
}
