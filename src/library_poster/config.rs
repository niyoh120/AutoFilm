use std::path::PathBuf;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    Card,
    Split,
    #[default]
    Collage,
    Blur,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    Random,
    DateCreated,
    #[default]
    DateLastContentAdded,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Resolution {
    P480,
    P720,
    P1080,
    Custom(u32, u32),
}

impl Default for Resolution {
    fn default() -> Self {
        Self::P1080
    }
}

impl Resolution {
    /// 将配置分辨率转换为图像库使用的宽高。
    ///
    /// 自定义尺寸会拒绝零值，避免创建无效画布。
    pub fn dimensions(self) -> Result<(u32, u32), String> {
        match self {
            Self::P480 => Ok((854, 480)),
            Self::P720 => Ok((1280, 720)),
            Self::P1080 => Ok((1920, 1080)),
            Self::Custom(0, _) | Self::Custom(_, 0) => {
                Err("自定义分辨率的宽高必须大于零".to_string())
            }
            Self::Custom(width, height) => Ok((width, height)),
        }
    }
}

#[derive(Deserialize, Serialize)]
struct CustomResolution {
    width: u32,
    height: u32,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum ResolutionValue {
    Preset(String),
    Custom { custom: CustomResolution },
}

impl Serialize for Resolution {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::P480 => ResolutionValue::Preset("480p".to_string()).serialize(serializer),
            Self::P720 => ResolutionValue::Preset("720p".to_string()).serialize(serializer),
            Self::P1080 => ResolutionValue::Preset("1080p".to_string()).serialize(serializer),
            Self::Custom(width, height) => ResolutionValue::Custom {
                custom: CustomResolution {
                    width: *width,
                    height: *height,
                },
            }
            .serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Resolution {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let resolution = match ResolutionValue::deserialize(deserializer)? {
            ResolutionValue::Preset(value) => match value.as_str() {
                "480p" => Self::P480,
                "720p" => Self::P720,
                "1080p" => Self::P1080,
                _ => return Err(de::Error::custom(format!("不支持的分辨率预设: {value}"))),
            },
            ResolutionValue::Custom { custom } => Self::Custom(custom.width, custom.height),
        };

        resolution.dimensions().map_err(de::Error::custom)?;
        Ok(resolution)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RenderConfig {
    #[serde(default)]
    pub style: Style,
    #[serde(default)]
    pub resolution: Resolution,
    #[serde(default = "default_blur_radius")]
    pub blur_radius: f32,
    #[serde(default = "default_color_strength")]
    pub color_strength: f32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            style: Style::default(),
            resolution: Resolution::default(),
            blur_radius: default_blur_radius(),
            color_strength: default_color_strength(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LibraryConfig {
    pub name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub style: Option<Style>,
    #[serde(default)]
    pub resolution: Option<Resolution>,
    #[serde(default)]
    pub sort: Sort,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    // 任务 ID 用于调度和日志识别。
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub cron: Option<String>,
    pub server: String,
    #[serde(default)]
    pub upload: bool,
    #[serde(default)]
    pub output_dir: Option<PathBuf>,
    pub title_font: PathBuf,
    pub subtitle_font: PathBuf,
    #[serde(default)]
    pub render: RenderConfig,
    #[serde(default)]
    pub libraries: Vec<LibraryConfig>,
}

fn default_blur_radius() -> f32 {
    50.0
}

fn default_color_strength() -> f32 {
    0.8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_preset_and_custom_resolutions() {
        let preset: Resolution = serde_yaml::from_str("1080p").unwrap();
        let custom: Resolution = serde_yaml::from_str(
            r#"
custom:
  width: 1600
  height: 900
"#,
        )
        .unwrap();

        assert_eq!(preset, Resolution::P1080);
        assert_eq!(preset.dimensions().unwrap(), (1920, 1080));
        assert_eq!(custom, Resolution::Custom(1600, 900));
        assert_eq!(custom.dimensions().unwrap(), (1600, 900));
    }

    #[test]
    fn serializes_custom_resolution_with_named_dimensions() {
        let yaml = serde_yaml::to_string(&Resolution::Custom(1600, 900)).unwrap();

        assert!(yaml.contains("custom:"));
        assert!(yaml.contains("width: 1600"));
        assert!(yaml.contains("height: 900"));
    }

    #[test]
    fn rejects_invalid_custom_resolution() {
        let zero = serde_yaml::from_str::<Resolution>(
            r#"
custom:
  width: 0
  height: 1080
"#,
        );

        assert!(zero.is_err());
    }

    #[test]
    fn parses_library_poster_task() {
        let config: Config = serde_yaml::from_str(
            r#"
id: 媒体库海报
server: 我的Jellyfin
upload: false
output_dir: /media/posters
title_font: /fonts/ch.ttf
subtitle_font: /fonts/en.otf
render:
  style: collage
  resolution: 1080p
libraries:
  - name: 电影
    title: 电影
    subtitle: MOVIE
    style: split
    resolution:
      custom:
        width: 1600
        height: 900
    sort: random
"#,
        )
        .unwrap();

        assert_eq!(config.render.style, Style::Collage);
        assert_eq!(config.libraries[0].style, Some(Style::Split));
        assert_eq!(
            config.libraries[0].resolution,
            Some(Resolution::Custom(1600, 900))
        );
        assert_eq!(config.libraries[0].sort, Sort::Random);
    }
}
