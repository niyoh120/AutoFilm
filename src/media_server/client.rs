use std::time::Duration;

use reqwest::{RequestBuilder, Url};
use serde::de::DeserializeOwned;

use super::config::Config;
use super::error::{Error, Result};
use super::models::{ImageKind, Library, MediaItem, QueryResult, User};

#[derive(Debug, Clone)]
pub struct Client {
    config: Config,
    base_url: Url,
    http: reqwest::Client,
}

impl Client {
    /// 创建媒体服务器客户端并统一处理基础 URL、超时和认证请求头。
    pub fn new(mut config: Config) -> Result<Self> {
        config.base_url = config.base_url.trim_end_matches('/').to_string();
        let base_url = Url::parse(&format!("{}/", config.base_url))
            .map_err(|err| Error::InvalidUrl(err.to_string()))?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout))
            .user_agent(format!("AutoFilm/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(Error::BuildClient)?;

        Ok(Self {
            config,
            base_url,
            http,
        })
    }

    pub fn id(&self) -> &str {
        &self.config.id
    }

    pub fn kind(&self) -> super::Kind {
        self.config.kind
    }

    /// 返回配置用户 ID；未配置时从服务器用户列表中选择首个用户。
    pub async fn resolve_user_id(&self) -> Result<String> {
        if let Some(user_id) = self
            .config
            .user_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            return Ok(user_id.to_string());
        }

        self.get_json::<Vec<User>>("Users", &[])
            .await?
            .into_iter()
            .next()
            .map(|user| user.id)
            .ok_or(Error::MissingUser)
    }

    /// 获取 Jellyfin/Emby 媒体库列表。
    pub async fn libraries(&self) -> Result<Vec<Library>> {
        Ok(self
            .get_json::<QueryResult<Library>>("Library/MediaFolders", &[])
            .await?
            .items)
    }

    /// 获取指定媒体库下的候选媒体项目。
    pub async fn items(
        &self,
        user_id: &str,
        library_id: &str,
        sort_by: &str,
        random_seed: Option<u64>,
        limit: usize,
    ) -> Result<Vec<MediaItem>> {
        let path = format!("Users/{user_id}/Items");
        let mut query = vec![
            ("ParentId", library_id.to_string()),
            ("Recursive", "true".to_string()),
            ("SortBy", sort_by.to_string()),
            ("SortOrder", "Descending".to_string()),
            (
                "IncludeItemTypes",
                "Movie,Series,Episode,MusicAlbum,Audio,MusicVideo,BoxSet".to_string(),
            ),
            ("Limit", limit.to_string()),
            (
                "Fields",
                "SeriesId,ParentBackdropItemId,ParentBackdropImageTags,BackdropImageTags"
                    .to_string(),
            ),
        ];
        if let Some(seed) = random_seed {
            query.push(("RandomSeed", seed.to_string()));
        }
        let query = query
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect::<Vec<_>>();

        Ok(self
            .get_json::<QueryResult<MediaItem>>(&path, &query)
            .await?
            .items)
    }

    /// 下载指定媒体项目的 Primary 或 Backdrop 图片。
    pub async fn download_image(
        &self,
        item_id: &str,
        kind: ImageKind,
        tag: Option<&str>,
    ) -> Result<Vec<u8>> {
        let path = format!("Items/{item_id}/Images/{}", kind.path());
        let query = tag.map(|tag| vec![("tag", tag)]).unwrap_or_default();
        let response = self
            .request(reqwest::Method::GET, &path, &query)?
            .send()
            .await?;
        Ok(Self::success(response).await?.bytes().await?.to_vec())
    }

    /// 使用媒体服务器要求的 Base64 请求体更新媒体库 Primary 图片。
    pub async fn upload_primary_image(&self, library_id: &str, encoded_png: &str) -> Result<()> {
        let path = format!("Items/{library_id}/Images/Primary");
        let response = self
            .request(reqwest::Method::POST, &path, &[])?
            .header(reqwest::header::CONTENT_TYPE, "image/png")
            .body(encoded_png.to_string())
            .send()
            .await?;
        Self::success(response).await?;
        Ok(())
    }

    async fn get_json<T>(&self, path: &str, query: &[(&str, &str)]) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let response = self
            .request(reqwest::Method::GET, path, query)?
            .send()
            .await?;
        Ok(Self::success(response).await?.json().await?)
    }

    fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<RequestBuilder> {
        let url = self
            .base_url
            .join(path)
            .map_err(|err| Error::InvalidUrl(err.to_string()))?;
        Ok(self
            .http
            .request(method, url)
            .header("X-Emby-Token", &self.config.api_key)
            .query(&[("api_key", self.config.api_key.as_str())])
            .query(query))
    }

    async fn success(response: reqwest::Response) -> Result<reqwest::Response> {
        if response.status().is_success() {
            return Ok(response);
        }

        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| status.canonical_reason().unwrap_or("").to_string());
        Err(Error::HttpStatus { status, body })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media_server::Kind;

    fn config(base_url: &str) -> Config {
        Config {
            id: "server".to_string(),
            kind: Kind::Jellyfin,
            base_url: base_url.to_string(),
            api_key: "secret".to_string(),
            user_id: Some("user".to_string()),
            timeout: 30,
        }
    }

    #[test]
    fn normalizes_base_url() {
        let client = Client::new(config("http://localhost:8096/")).unwrap();

        assert_eq!(client.base_url.as_str(), "http://localhost:8096/");
        assert_eq!(client.id(), "server");
    }

    #[tokio::test]
    async fn uses_configured_user_id_without_request() {
        let client = Client::new(config("http://localhost:8096")).unwrap();

        assert_eq!(client.resolve_user_id().await.unwrap(), "user");
    }
}
