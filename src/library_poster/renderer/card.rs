use image::{DynamicImage, Rgba, RgbaImage};
use imageproc::geometric_transformations::{Interpolation, rotate_about_center};

use super::utils::{
    adjust_brightness, apply_rounded_corners, cover, dominant_color, draw_titles_wrapped,
    optimized_blur, overlay_with_shadow, tint,
};
use super::{Fonts, Result};
use crate::library_poster::RenderConfig;

/// 生成右侧 iMessage 式多卡片堆叠、左侧标题的卡片风格。
pub fn render(
    images: &[DynamicImage],
    title: &str,
    subtitle: &str,
    fonts: &Fonts,
    config: &RenderConfig,
    dimensions: (u32, u32),
) -> Result<RgbaImage> {
    let source = images.first().ok_or(super::Error::MissingImage)?;
    let (width, height) = dimensions;
    let theme = dominant_color(source);
    let mut canvas = cover(source, width, height);
    canvas = optimized_blur(
        &canvas,
        config.blur_radius.max(0.0) * height as f32 / 1080.0,
    );
    tint(
        &mut canvas,
        adjust_brightness(theme, 0.78),
        config.color_strength.clamp(0.0, 1.0),
    );

    let card_height = (height as f32 * 0.76) as u32;
    let card_width = (card_height as f32 * 0.70) as u32;
    let center_x = width as f32 * 0.745;
    let center_y = height as f32 * 0.51;
    let horizontal_step = width as f32 * 0.060;

    // 根据实际素材数量使用 7/5/3/1 层，避免重复图片填满卡片堆。
    let layers = layers_for_count(images.len(), horizontal_step);

    for (depth, layer) in layers.iter().copied().enumerate() {
        let layer_width = (card_width as f32 * layer.scale) as u32;
        let layer_height = (card_height as f32 * layer.scale) as u32;
        let card = prepare_card(
            &images[layer.image_index],
            layer_width,
            layer_height,
            layer.darkness,
            layer.opacity,
        );
        let rotated = rotate_with_padding(&card, layer.angle);
        let vertical_offset = (1.0 - layer.scale) * height as f32 * 0.18;
        let shadow_strength = 88 + depth as u8 * 12;
        overlay_with_shadow(
            &mut canvas,
            &rotated,
            (center_x + layer.offset - rotated.width() as f32 / 2.0) as i64,
            (center_y + vertical_offset - rotated.height() as f32 / 2.0) as i64,
            (height as f32 * 0.010) as i64,
            (height as f32 * 0.014) as i64,
            height as f32 * 0.012,
            shadow_strength,
        );
    }

    draw_titles_wrapped(
        &mut canvas,
        title,
        subtitle,
        fonts,
        (width as f32 * 0.25) as i32,
        (height as f32 * 0.37) as i32,
        height as f32 * 0.14,
        height as f32 * 0.055,
        Rgba([255, 255, 255, 235]),
        (width as f32 * 0.40) as i32,
    );
    Ok(canvas)
}

#[derive(Clone, Copy)]
struct Layer {
    image_index: usize,
    offset: f32,
    angle: f32,
    scale: f32,
    darkness: f32,
    opacity: u8,
}

fn layers_for_count(image_count: usize, horizontal_step: f32) -> Vec<Layer> {
    let visible_count = match image_count {
        7.. => 7,
        5.. => 5,
        3.. => 3,
        _ => 1,
    };
    let side_count = (visible_count - 1) / 2;
    let mut layers = Vec::with_capacity(visible_count);

    // 先从最外侧绘制到内侧，最后绘制中央主卡。
    for distance in (1..=side_count).rev() {
        let progress = distance as f32 / side_count.max(1) as f32;
        let scale = 1.0 - progress * 0.14;
        let darkness = progress * 0.36;
        let opacity = (255.0 - progress * 58.0) as u8;
        let angle = progress * 9.0;
        let offset = horizontal_step * distance as f32 * 0.78;
        let left_index = distance * 2 - 1;
        let right_index = distance * 2;

        layers.push(Layer {
            image_index: left_index,
            offset: -offset,
            angle: -angle,
            scale,
            darkness,
            opacity,
        });
        layers.push(Layer {
            image_index: right_index,
            offset,
            angle,
            scale,
            darkness,
            opacity,
        });
    }
    layers.push(Layer {
        image_index: 0,
        offset: 0.0,
        angle: 0.0,
        scale: 1.0,
        darkness: 0.0,
        opacity: 255,
    });
    layers
}

fn prepare_card(
    source: &DynamicImage,
    width: u32,
    height: u32,
    darkness: f32,
    opacity: u8,
) -> RgbaImage {
    let mut card = cover(source, width, height);
    if darkness > 0.0 {
        tint(&mut card, Rgba([18, 18, 22, 255]), darkness);
    }
    apply_rounded_corners(&mut card, (width as f32 * 0.10) as u32);
    if opacity < 255 {
        for pixel in card.pixels_mut() {
            pixel[3] = ((pixel[3] as u16 * opacity as u16) / 255) as u8;
        }
    }
    card
}

/// 先给卡片补透明边距再旋转，避免旋转后的圆角被原始边界裁掉。
fn rotate_with_padding(card: &RgbaImage, angle: f32) -> RgbaImage {
    if angle == 0.0 {
        return card.clone();
    }
    let padding = (card.width().max(card.height()) as f32 * 0.10) as u32;
    let mut padded = RgbaImage::from_pixel(
        card.width() + padding * 2,
        card.height() + padding * 2,
        Rgba([0, 0, 0, 0]),
    );
    image::imageops::overlay(&mut padded, card, i64::from(padding), i64::from(padding));
    rotate_about_center(
        &padded,
        angle.to_radians(),
        Interpolation::Bicubic,
        Rgba([0, 0, 0, 0]),
    )
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, Rgb, RgbImage};

    use super::*;

    #[test]
    fn prepares_rounded_translucent_back_card() {
        let source = DynamicImage::ImageRgb8(RgbImage::from_pixel(400, 600, Rgb([80, 160, 220])));

        let card = prepare_card(&source, 200, 300, 0.25, 200);

        assert_eq!(card.dimensions(), (200, 300));
        assert_eq!(card.get_pixel(0, 0)[3], 0);
        assert_eq!(card.get_pixel(100, 150)[3], 200);
    }

    #[test]
    fn rotating_card_keeps_transparent_padding() {
        let card = RgbaImage::from_pixel(200, 300, Rgba([80, 160, 220, 255]));

        let rotated = rotate_with_padding(&card, 8.0);

        assert!(rotated.width() > card.width());
        assert!(rotated.height() > card.height());
        assert_eq!(rotated.get_pixel(0, 0)[3], 0);
    }

    #[test]
    fn card_layers_fall_back_to_odd_counts() {
        for (available, expected) in [
            (1, 1),
            (2, 1),
            (3, 3),
            (4, 3),
            (5, 5),
            (6, 5),
            (7, 7),
            (10, 7),
        ] {
            let layers = layers_for_count(available, 40.0);

            assert_eq!(layers.len(), expected);
            assert!(layers.iter().all(|layer| layer.image_index < available));
        }
    }
}
