mod blur;
mod card;
mod collage;
mod split;
mod utils;

use ab_glyph::FontArc;
use image::{DynamicImage, RgbaImage};
use thiserror::Error;

use super::{RenderConfig, Style};

#[derive(Clone)]
pub struct Fonts {
    pub title: FontArc,
    pub subtitle: FontArc,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("海报渲染至少需要一张素材图片")]
    MissingImage,

    #[error("分辨率配置无效: {0}")]
    InvalidResolution(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// 根据风格配置生成最终媒体库封面。
pub fn render(
    images: &[DynamicImage],
    title: &str,
    subtitle: &str,
    fonts: &Fonts,
    config: &RenderConfig,
) -> Result<RgbaImage> {
    let dimensions = config
        .resolution
        .dimensions()
        .map_err(Error::InvalidResolution)?;

    match config.style {
        Style::Card => card::render(images, title, subtitle, fonts, config, dimensions),
        Style::Split => split::render(images, title, subtitle, fonts, config, dimensions),
        Style::Collage => collage::render(images, title, subtitle, fonts, config, dimensions),
        Style::Blur => blur::render(images, title, subtitle, fonts, config, dimensions),
    }
}

#[cfg(test)]
mod tests {
    use ab_glyph::FontArc;
    use image::{DynamicImage, Rgb, RgbImage};

    use super::*;
    use crate::library_poster::Resolution;

    fn fonts() -> Fonts {
        Fonts {
            title: FontArc::try_from_slice(include_bytes!("../../fonts/ch.ttf")).unwrap(),
            subtitle: FontArc::try_from_slice(include_bytes!("../../fonts/en.otf")).unwrap(),
        }
    }

    fn images() -> Vec<DynamicImage> {
        [[220, 80, 120], [40, 120, 220]]
            .into_iter()
            .map(|color| DynamicImage::ImageRgb8(RgbImage::from_pixel(600, 900, Rgb(color))))
            .collect()
    }

    #[test]
    fn renders_all_styles_at_requested_resolution() {
        for style in [Style::Card, Style::Split, Style::Collage, Style::Blur] {
            let config = RenderConfig {
                style,
                resolution: Resolution::Custom {
                    width: 640,
                    height: 360,
                },
                ..RenderConfig::default()
            };

            let poster = render(&images(), "电影", "MOVIE", &fonts(), &config).unwrap();

            assert_eq!(poster.dimensions(), (640, 360));
        }
    }
}
