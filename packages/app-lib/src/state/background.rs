//! Launcher background configuration, used for both the global background
//! (stored on [`Settings`](super::Settings)) and per-instance overrides
//! (stored with the instance launch overrides).

use serde::{Deserialize, Serialize};

pub const BACKGROUND_MAX_DIM: u8 = 100;
pub const BACKGROUND_MAX_BLUR: u8 = 32;
const DEFAULT_DIM: u8 = 40;
const DEFAULT_BLUR: u8 = 8;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BackgroundSource {
    #[default]
    None,
    /// A solid colour, as `#rgb` or `#rrggbb`.
    Color { color: String },
    /// A linear gradient between two `#rgb`/`#rrggbb` colours at a CSS angle
    /// in degrees.
    Gradient {
        from: String,
        to: String,
        angle: u16,
    },
    /// An image previously copied into the backgrounds cache by
    /// [`crate::api::background::cache_image`].
    Image { path: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct BackgroundConfig {
    #[serde(default)]
    pub source: BackgroundSource,
    /// How far the background is darkened, 0-100. Panel opacity follows this.
    #[serde(default = "default_dim")]
    pub dim: u8,
    /// Blur applied behind panels, in px, 0-32.
    #[serde(default = "default_blur")]
    pub blur: u8,
}

fn default_dim() -> u8 {
    DEFAULT_DIM
}

fn default_blur() -> u8 {
    DEFAULT_BLUR
}

impl Default for BackgroundConfig {
    fn default() -> Self {
        Self {
            source: BackgroundSource::None,
            dim: DEFAULT_DIM,
            blur: DEFAULT_BLUR,
        }
    }
}

impl BackgroundConfig {
    /// Clamps the sliders and drops sources with malformed colours, so nothing
    /// unvalidated from the frontend reaches the database or a CSS value.
    pub fn sanitized(mut self) -> Self {
        self.dim = self.dim.min(BACKGROUND_MAX_DIM);
        self.blur = self.blur.min(BACKGROUND_MAX_BLUR);
        self.source = match self.source {
            BackgroundSource::Color { color } if is_hex_color(&color) => {
                BackgroundSource::Color { color }
            }
            BackgroundSource::Gradient { from, to, angle }
                if is_hex_color(&from) && is_hex_color(&to) =>
            {
                BackgroundSource::Gradient {
                    from,
                    to,
                    angle: angle % 360,
                }
            }
            BackgroundSource::Image { path } if !path.is_empty() => {
                BackgroundSource::Image { path }
            }
            _ => BackgroundSource::None,
        };
        self
    }

    pub fn image_path(&self) -> Option<&str> {
        match &self.source {
            BackgroundSource::Image { path } => Some(path),
            _ => None,
        }
    }
}

fn is_hex_color(value: &str) -> bool {
    let Some(digits) = value.strip_prefix('#') else {
        return false;
    };
    matches!(digits.len(), 3 | 6)
        && digits.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitized_clamps_sliders() {
        let config = BackgroundConfig {
            source: BackgroundSource::None,
            dim: 250,
            blur: 200,
        }
        .sanitized();
        assert_eq!(config.dim, BACKGROUND_MAX_DIM);
        assert_eq!(config.blur, BACKGROUND_MAX_BLUR);
    }

    #[test]
    fn sanitized_rejects_malformed_colours() {
        for color in ["red", "#12", "#12345", "#gggggg", "url(x)", ""] {
            let config = BackgroundConfig {
                source: BackgroundSource::Color {
                    color: color.to_string(),
                },
                ..Default::default()
            }
            .sanitized();
            assert_eq!(config.source, BackgroundSource::None, "{color}");
        }

        let config = BackgroundConfig {
            source: BackgroundSource::Gradient {
                from: "#fff".to_string(),
                to: "not-a-colour".to_string(),
                angle: 90,
            },
            ..Default::default()
        }
        .sanitized();
        assert_eq!(config.source, BackgroundSource::None);
    }

    #[test]
    fn sanitized_keeps_valid_sources_and_wraps_angle() {
        let config = BackgroundConfig {
            source: BackgroundSource::Gradient {
                from: "#54ff54".to_string(),
                to: "#55f".to_string(),
                angle: 450,
            },
            ..Default::default()
        }
        .sanitized();
        assert_eq!(
            config.source,
            BackgroundSource::Gradient {
                from: "#54ff54".to_string(),
                to: "#55f".to_string(),
                angle: 90,
            }
        );
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let config: BackgroundConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config, BackgroundConfig::default());
    }
}
