//! The page's size: desktop, tablet, phone, or an exact size. A
//! tablet or phone also introduces itself as iOS Safari (the engine the system WebView is on
//! macOS), so sites serve their mobile layout; there is no touch emulation.

#[derive(Debug, Clone, PartialEq)]
pub struct Viewport {
    pub width: f64,
    pub height: f64,
    /// Overrides the default user agent (a tablet's or phone's).
    pub user_agent: Option<&'static str>,
}

const IPHONE: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 \
    (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1";
const IPAD: &str = "Mozilla/5.0 (iPad; CPU OS 18_5 like Mac OS X) AppleWebKit/605.1.15 \
    (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1";

impl Viewport {
    pub const DESKTOP: Viewport = Viewport {
        width: 1280.0,
        height: 860.0,
        user_agent: None,
    };

    /// `desktop`, `tablet`, `mobile` (or `phone`), or `WIDTHxHEIGHT` such as `1440x900`.
    pub fn parse(text: &str) -> Result<Viewport, String> {
        match text.trim().to_ascii_lowercase().as_str() {
            "desktop" => Ok(Self::DESKTOP),
            "tablet" | "ipad" => Ok(Viewport {
                width: 820.0,
                height: 1180.0,
                user_agent: Some(IPAD),
            }),
            "mobile" | "phone" | "iphone" => Ok(Viewport {
                width: 390.0,
                height: 844.0,
                user_agent: Some(IPHONE),
            }),
            other => {
                let (w, h) = other.split_once('x').ok_or_else(|| {
                    format!(
                        "unknown viewport {text:?}: use desktop, tablet, mobile or WIDTHxHEIGHT"
                    )
                })?;
                let size = |n: &str| {
                    n.trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|n| (200.0..=4000.0).contains(n))
                };
                match (size(w), size(h)) {
                    (Some(width), Some(height)) => Ok(Viewport {
                        width,
                        height,
                        user_agent: None,
                    }),
                    _ => Err(format!(
                        "viewport {text:?}: each side must be 200–4000 pixels"
                    )),
                }
            }
        }
    }

    pub fn label(&self) -> String {
        format!("{}×{}", self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_and_exact_sizes() {
        assert_eq!(Viewport::parse("Desktop"), Ok(Viewport::DESKTOP));
        let phone = Viewport::parse("mobile").unwrap();
        assert_eq!((phone.width, phone.height), (390.0, 844.0));
        assert!(phone.user_agent.unwrap().contains("iPhone"));
        assert!(
            Viewport::parse("tablet")
                .unwrap()
                .user_agent
                .unwrap()
                .contains("iPad")
        );
        assert_eq!(Viewport::parse("1440x900").unwrap().label(), "1440×900");
        assert!(Viewport::parse("10x10").is_err());
        assert!(Viewport::parse("watch").is_err());
    }
}
