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
    use rand::seq::SliceRandom;

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

    #[test]
    #[ignore]
    fn generates_readme_previews() {
        let previews = [
            (Style::Card, "动漫", "ANIME", "img/海报/动漫", "card"),
            (Style::Split, "电影", "MOVIE", "img/海报/电影", "split"),
            (Style::Collage, "动漫", "ANIME", "img/海报/动漫", "collage"),
            (
                Style::Blur,
                "电视剧",
                "TV SERIES",
                "img/海报/电视剧",
                "blur",
            ),
        ];
        std::fs::create_dir_all("img/library-poster").unwrap();

        for (style, title, subtitle, source_dir, filename) in previews {
            let mut paths = std::fs::read_dir(source_dir)
                .unwrap()
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .and_then(|extension| extension.to_str())
                        .is_some_and(|extension| {
                            matches!(
                                extension.to_ascii_lowercase().as_str(),
                                "jpg" | "jpeg" | "png"
                            )
                        })
                })
                .collect::<Vec<_>>();
            paths.shuffle(&mut rand::rng());
            paths.truncate(if matches!(style, Style::Collage) {
                9
            } else {
                1
            });
            let images = paths
                .iter()
                .map(|path| image::open(path).unwrap())
                .collect::<Vec<_>>();
            let config = RenderConfig {
                style,
                resolution: Resolution::P1080,
                ..RenderConfig::default()
            };
            let poster = render(&images, title, subtitle, &fonts(), &config).unwrap();
            poster
                .save(format!("img/library-poster/{filename}.png"))
                .unwrap();
        }
    }
}
