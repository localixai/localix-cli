//! The page the browser shows when `lxsurf login` finishes: Localix's sign-in page (the bird in the
//! wind, from Localix's `auth/oauth-page.ts`), success or failure.

const LOGO: &str = include_str!("../assets/logo-dark.svg");

/// Streaks of wind behind the bird; the same pseudo-random layout on every render.
fn wind_lines(count: usize) -> String {
    (0..count)
        .map(|i| {
            let random = |seed: f64| {
                let x = ((i as f64) * 12.9898 + seed * 78.233).sin() * 43758.5453;
                ((x % 1.0) + 1.0) % 1.0
            };
            let duration = 1.4 + random(1.0) * 1.8;
            let opacity = 0.14 + random(5.0) * 0.28;
            format!(
                "<i style=\"--y:{:.1}px;--w:{:.1}px;--delay:{:.2}s;--duration:{duration:.2}s;\
                 --opacity:{opacity:.2};--wind-opacity:{opacity:.2};--thickness:{:.1}px\"></i>",
                (random(2.0) - 0.5) * 200.0,
                120.0 + random(3.0) * 200.0,
                -random(4.0) * duration,
                1.2 + random(6.0) * 2.0,
            )
        })
        .collect()
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

const STYLE: &str = r#"
:root { --text: #fafafa; --text-dim: #a1a1aa; --page-bg: #09090b;
  --font-sans: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
  --font-mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; }
* { box-sizing: border-box; }
html { color-scheme: dark; }
body { margin: 0; min-height: 100vh; display: flex; align-items: center; justify-content: center; padding: 24px;
  background: var(--page-bg); color: var(--text); font-family: var(--font-sans); text-align: center; }
main { width: 100%; max-width: 560px; display: flex; flex-direction: column; align-items: center; }
.bird-stage { position: relative; width: 176px; height: 176px; display: grid; place-items: center; margin: 0 auto 20px;
  animation: bird-intro .7s cubic-bezier(.2,.8,.2,1) both; }
.wind { position: absolute; inset: 0; overflow: visible; pointer-events: none; }
.wind i { position: absolute; top: calc(50% + var(--y)); right: -40%; width: var(--w); height: var(--thickness);
  border-radius: 999px; background: linear-gradient(90deg, transparent 5%, rgba(225,228,240,var(--opacity)) 50%, transparent 95%);
  animation: wind-sweep var(--duration) var(--delay) linear infinite; }
.brand-mark { position: relative; z-index: 1; width: 80px; height: 80px; display: block;
  filter: drop-shadow(0 10px 26px rgba(153, 181, 255, .16)); animation: bird-drift 6s ease-in-out infinite; }
.brand-mark svg { width: 100%; height: 100%; display: block; }
h1 { margin: 0 0 10px; font-size: 28px; line-height: 1.15; font-weight: 650; }
p { margin: 0; line-height: 1.7; color: var(--text-dim); font-size: 15px; }
.details { margin-top: 16px; font-family: var(--font-mono); font-size: 13px; color: var(--text-dim);
  white-space: pre-wrap; word-break: break-word; }
@keyframes bird-intro { from { opacity: 0; transform: scale(.95); } to { opacity: 1; transform: scale(1); } }
@keyframes bird-drift { 0%, 50%, 100% { transform: translate3d(0,0,0) scale(1); }
  25% { transform: translate3d(0,-6px,0) scale(1.01); } 75% { transform: translate3d(0,4px,0) scale(.99); } }
@keyframes wind-sweep { 0% { opacity: 0; transform: translate3d(0,0,0); } 18%, 72% { opacity: var(--wind-opacity, .2); }
  100% { opacity: 0; transform: translate3d(-600px,0,0); } }
@media (prefers-reduced-motion: reduce) { .bird-stage, .brand-mark, .wind i { animation: none; } .wind i { display: none; } }
"#;

fn page(heading: &str, message: &str, details: Option<&str>) -> String {
    let logo = LOGO
        .trim_start_matches(|c| c != '<')
        .split_once("?>")
        .map_or(LOGO, |(_, svg)| svg);
    let details = details
        .map(|details| format!("<div class=\"details\">{}</div>", escape(details)))
        .unwrap_or_default();
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\" />\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />\n\
         <title>{heading}</title>\n<style>{STYLE}</style>\n</head>\n<body>\n<main>\n\
         <div class=\"bird-stage\" aria-label=\"Localix\"><div class=\"wind\">{wind}</div>\
         <span class=\"brand-mark\">{logo}</span></div>\n<h1>{heading}</h1>\n<p>{message}</p>\n{details}\n\
         </main>\n</body>\n</html>",
        heading = escape(heading),
        message = escape(message),
        wind = wind_lines(24),
    )
}

pub fn success() -> String {
    page(
        "Authentication successful",
        "Localix Surf is signed in to OpenRouter. You can close this tab and return to the terminal.",
        None,
    )
}

pub fn failure(message: &str, details: Option<&str>) -> String {
    page("Authentication failed", message, details)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_carries_the_logo_and_escapes_details() {
        let ok = success();
        assert!(
            ok.contains("<svg") && !ok.contains("<?xml"),
            "the logo is inline SVG"
        );
        assert!(ok.contains("Authentication successful"));
        let failed = failure("OpenRouter sign-in failed.", Some("<script>x</script>"));
        assert!(failed.contains("&lt;script&gt;") && !failed.contains("<script>x"));
        assert_eq!(wind_lines(3), wind_lines(3), "the layout is stable");
    }
}
