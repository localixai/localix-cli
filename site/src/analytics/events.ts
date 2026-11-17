// Every event the site sends beyond Google's own (page views, scrolls, outbound clicks, downloads),
// with what each may carry. A value is a member of a closed set or a boolean, never free text:
// nothing a visitor typed, no address but the page's own.
import { INSTALL } from '../links';

export type AnalyticsEvents = {
  /** The landing's install command copied; `method` is its tab's label from `INSTALL`. */
  install_copied: { method: string };
  /** A docs code block copied. */
  code_copied: Record<string, never>;
};

export type AnalyticsEvent = keyof AnalyticsEvents;

/** Each string parameter's closed set; a value outside it drops the event. */
const SETS: { readonly [P in string]?: () => readonly string[] } = {
  method: () => INSTALL.map(([label]) => label),
};

/** The event's parameters as GA4 takes them, or undefined when one is outside its set. */
export function wireParams(params: Record<string, string | boolean>): Record<string, string> | undefined {
  const wire: Record<string, string> = {};
  for (const [key, value] of Object.entries(params)) {
    if (typeof value === 'boolean') {
      wire[key] = String(value);
      continue;
    }
    if (!SETS[key]?.().includes(value)) return undefined;
    wire[key] = value;
  }
  return wire;
}
