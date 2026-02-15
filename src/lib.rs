use std::collections::HashMap;

use error::JackettError;
use extism_pdk::{http, log, plugin_fn, FnResult, HttpRequest, Json, LogLevel, WithReturnCode};
#[cfg(target_arch = "wasm32")]
use extism_pdk::info;

use rs_plugin_common_interfaces::{lookup::{RsLookupQuery, RsLookupSourceResult, RsLookupWrapper}, request::{RsRequest, RsRequestPluginRequest, RsRequestStatus}, CredentialType, CustomParam, CustomParamTypes, PluginInformation, PluginType};
use rs_torrent_magnet::magnet_from_torrent;
use serde::{Deserialize, Serialize};

use urlencoding::encode;
use unidecode::unidecode;

pub mod error;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "PascalCase")] 
pub struct JackettResults {
    pub results: Vec<JackettResult>,
}

impl JackettResults {
    pub fn censor(&mut self, token: &str) {
        for result in &mut self.results {
            if let Some(link) = &mut result.link {
                result.link = Some(link.replace(token, ""));
            }
        }
    }
}

const JACKET_MIME: &str = "jackett/torrent";
const DEFAULT_BASE_URL: &str = "http://127.0.0.1:9117";

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "PascalCase")] 
pub struct JackettResult {
    pub title: String,
    pub tracker: Option<String>,
    pub link: Option<String>,
    pub magnet_uri: Option<String>,
    pub size: Option<u64>,
    pub seeders: u64,
    pub tmdb: Option<u64>,
    pub imdb: Option<String>
}

impl TryFrom<JackettResult> for RsRequest {
    type Error = JackettError;
    
    fn try_from(value: JackettResult) -> Result<Self, JackettError> {
        let mut request = if let Some(magnet) = value.magnet_uri {
            RsRequest { upload_id: None, url: magnet, mime: Some("applications/x-bittorrent".to_owned()), size: value.size, filename: Some(value.title), referer: value.tracker, status: RsRequestStatus::Unprocessed, permanent: true, ..Default::default() }
        } else if let Some(url) = value.link {
            RsRequest { upload_id: None, url, mime: Some(JACKET_MIME.to_owned()), size: value.size, filename: Some(value.title), referer: value.tracker, status: RsRequestStatus::Unprocessed, permanent: false, ..Default::default() }
        } else {
            return Err(JackettError::NoLink(value))
        };
        request.parse_filename();
        Ok(request)
    }
}

#[plugin_fn]
pub fn infos() -> FnResult<Json<PluginInformation>> {
    Ok(Json(
        PluginInformation {
            name: "jackett_lookup".into(),
            capabilities: vec![PluginType::Lookup, PluginType::Request],
            version: 1,
            interface_version: 1,
            publisher: "neckaros".into(),
            description: "fetch possible movies or episode with the Jackett API".into(),
            credential_kind: Some(CredentialType::Token),
            settings: vec![
                CustomParam {
                    name: "base_url".into(),
                    param: CustomParamTypes::Url(Some(DEFAULT_BASE_URL.into())),
                    description: Some("Jackett server base URL".into()),
                    required: false,
                }
            ],
            ..Default::default()
        }
    ))
}


pub fn get_request(base_url: Option<&str>, token: String, params: HashMap<&str, String>) -> HttpRequest {
    let base_url = base_url.unwrap_or(DEFAULT_BASE_URL);
    let url = format!("{}/api/v2.0/indexers/all/results", base_url.trim_end_matches('/'));

    let params_string = params.into_iter().map(|(key, value)| format!("{}={}", key, encode(&value))).collect::<Vec<_>>().join("&");
    #[cfg(target_arch = "wasm32")]
    info!("{}?apikey={}&{}", url, token, params_string);
    HttpRequest {
        url: format!("{}?apikey={}&{}", url, token, params_string),
        headers: Default::default(),
        method: Some("GET".into()),
    }
}

#[plugin_fn]
pub fn lookup(Json(lookup): Json<RsLookupWrapper>) -> FnResult<Json<RsLookupSourceResult>> {
    if let Some(token) = lookup.credential.and_then(|l| l.password) {
        let base_url = lookup.params
            .as_ref()
            .and_then(|p| p.get("base_url"))
            .map(|s| s.as_str());

        if let RsLookupQuery::Episode(episode_query) = lookup.query {
            let name = episode_query.name
                .ok_or_else(|| WithReturnCode::new(extism_pdk::Error::msg("Not supported"), 404))?;
            let q =  if let Some(number) = episode_query.number {
                format!("{} s{:02}e{:02}", unidecode(&name), episode_query.season, number)
            } else {
                format!("{} s{:02}", unidecode(&name), episode_query.season)
            };
            let params = HashMap::from([("t", "tvsearch".to_owned()),("Query", q)]);

            let request = get_request(base_url, token.clone(), params);

            let res = http::request::<()>(&request, None);
            match res {
                Ok(res) if res.status_code() >= 200 && res.status_code() < 300 => {
                    match res.json::<JackettResults>() {
                        Ok(r) => {
                            let requests: Vec<RsRequest> = r.results.into_iter().filter_map(|t| RsRequest::try_from(t).ok()).map(|mut r| {
                                r.url = r.url.replace(&token, "#token#");
                                r
                            } ).collect();
                            Ok(Json(RsLookupSourceResult::Requests(requests)))
                        }
                        Err(e) => {
                            log!(LogLevel::Error, "JSON parse error for episode lookup: {}", e);
                            Err(WithReturnCode::new(e, 500))
                        }
                    }
                }
                Ok(res) => {
                    log!(LogLevel::Error, "HTTP error ({}) {}: {}", request.url.replace(&token, "#token#"), res.status_code(), String::from_utf8_lossy(&res.body()));
                    Err(WithReturnCode::new(extism_pdk::Error::msg(format!("HTTP error: {}", res.status_code())), res.status_code() as i32))
                }
                Err(e) => {
                    log!(LogLevel::Error, "HTTP request failed for episode lookup: {}", e);
                    Err(WithReturnCode(e, 500))
                }
            }
        } else if let RsLookupQuery::Movie(movie_query) = lookup.query {
            let name = movie_query.name
                .ok_or_else(|| WithReturnCode::new(extism_pdk::Error::msg("Not supported"), 404))?;
            let params = HashMap::from([("t", "movie".to_owned()), ("Query", unidecode(&name))]);

            let request = get_request(base_url, token.clone(), params);

            let res = http::request::<()>(&request, None);
            match res {
                Ok(res) if res.status_code() >= 200 && res.status_code() < 300 => {
                    match res.json::<JackettResults>() {
                        Ok(r) => {
                            let requests: Vec<RsRequest> = r.results.into_iter().filter_map(|t| RsRequest::try_from(t).ok()).map(|mut r| {
                                r.url = r.url.replace(&token, "#token#");
                                r
                            } ).collect();
                            Ok(Json(RsLookupSourceResult::Requests(requests)))
                        }
                        Err(e) => {
                            log!(LogLevel::Error, "JSON parse error for movie lookup: {}", e);
                            Err(WithReturnCode::new(e, 500))
                        }
                    }
                }
                Ok(res) => {
                    log!(LogLevel::Error, "HTTP error ({}) {}: {}", request.url.replace(&token, "#token#"), res.status_code(), String::from_utf8_lossy(&res.body()));
                    Err(WithReturnCode::new(extism_pdk::Error::msg(format!("HTTP error: {}", res.status_code())), res.status_code() as i32))
                }
                Err(e) => {
                    log!(LogLevel::Error, "HTTP request failed for movie lookup: {}", e);
                    Err(WithReturnCode(e, 500))
                }
            }
        } else {
            Ok(Json(RsLookupSourceResult::NotApplicable))
        }
        
    } else {
        Err(WithReturnCode::new(extism_pdk::Error::msg("Need token"), 401))
    }
}



#[plugin_fn]
pub fn process(Json(request): Json<RsRequestPluginRequest>) -> FnResult<Json<RsRequest>> {
    if request.request.mime == Some(JACKET_MIME.to_owned()) {
        if let Some(token) = request.credential.and_then(|l| l.password) {
            let httprequest = HttpRequest {
                url: request.request.url.replace("#token#", token.as_str()),
                headers: Default::default(),
                method: Some("GET".into()),
            };
            let res = http::request::<()>(&httprequest, None);
            match res {
                Ok(res) if res.status_code() >= 200 && res.status_code() < 300 => {
                    let encoded = res.body();
                    let magnet = magnet_from_torrent(encoded)
                        .map_err(|e| WithReturnCode::new(extism_pdk::Error::msg(format!("Failed to parse torrent: {}", e)), 500))?;
                    let mut final_request = request.request.clone();
                    final_request.url = magnet;
                    final_request.status = RsRequestStatus::Intermediate;
                    final_request.permanent = true;
                    final_request.mime = Some("applications/x-bittorrent".to_owned());
                    Ok(Json(final_request))
                }
                Ok(res) => {
                    log!(LogLevel::Error, "HTTP error (process) ({}) {}: {}", request.request.url, res.status_code(), String::from_utf8_lossy(&res.body()));
                    Err(WithReturnCode::new(extism_pdk::Error::msg(format!("HTTP error: {}", res.status_code())), res.status_code() as i32))
                }
                Err(e) => {
                    log!(LogLevel::Error, "HTTP request failed (process) for {}: {}", request.request.url, e);
                    Err(WithReturnCode::new(extism_pdk::Error::msg("Request failed"), 500))
                }
            }
        } else {
            Err(WithReturnCode::new(extism_pdk::Error::msg("Need token"), 401))
        }
    } else {
        Err(WithReturnCode::new(extism_pdk::Error::msg("Not supported"), 404))
    }
}


#[plugin_fn]
pub fn request_permanent(Json(request): Json<RsRequestPluginRequest>) -> FnResult<Json<RsRequest>> {
    if request.request.mime == Some(JACKET_MIME.to_owned()) {
        if let Some(token) = request.credential.and_then(|l| l.password) {
            let httprequest = HttpRequest {
                url: request.request.url.replace("#token#", token.as_str()),
                headers: Default::default(),
                method: Some("GET".into()),
            };
            let res = http::request::<()>(&httprequest, None);
            match res {
                Ok(res) if res.status_code() >= 200 && res.status_code() < 300 => {
                    let encoded = res.body();
                    let magnet = magnet_from_torrent(encoded)
                        .map_err(|e| WithReturnCode::new(extism_pdk::Error::msg(format!("Failed to parse torrent: {}", e)), 500))?;
                    let mut final_request = request.request.clone();
                    final_request.url = magnet;
                    final_request.status = RsRequestStatus::Unprocessed;
                    final_request.permanent = true;
                    final_request.mime = Some("applications/x-bittorrent".to_owned());
                    Ok(Json(final_request))
                }
                Ok(res) => {
                    log!(LogLevel::Error, "HTTP error (request_permanent) ({}) {}: {}", request.request.url, res.status_code(), String::from_utf8_lossy(&res.body()));
                    Err(WithReturnCode::new(extism_pdk::Error::msg(format!("HTTP error: {}", res.status_code())), res.status_code() as i32))
                }
                Err(e) => {
                    log!(LogLevel::Error, "HTTP request failed (request_permanent) for {}: {}", request.request.url, e);
                    Err(WithReturnCode::new(extism_pdk::Error::msg("Request failed"), 500))
                }
            }
        } else {
            Err(WithReturnCode::new(extism_pdk::Error::msg("Need token"), 401))
        }
    } else {
        Err(WithReturnCode::new(extism_pdk::Error::msg("Not supported"), 404))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request() {
        let request = get_request(None, "testtoken".to_owned(), HashMap::from([("testparam1", "A".to_owned()), ("testparam2", "B".to_owned())]));

        // HashMap iteration order is not guaranteed, so we check parts instead
        assert!(request.url.starts_with("http://127.0.0.1:9117/api/v2.0/indexers/all/results?apikey=testtoken&"));
        assert!(request.url.contains("testparam1=A"));
        assert!(request.url.contains("testparam2=B"));
    }

    #[test]
    fn request_with_custom_base_url() {
        let request = get_request(Some("http://192.168.1.100:9117"), "testtoken".to_owned(), HashMap::from([("t", "tvsearch".to_owned())]));

        assert_eq!(request.url, "http://192.168.1.100:9117/api/v2.0/indexers/all/results?apikey=testtoken&t=tvsearch");
    }

    #[test]
    fn request_with_trailing_slash() {
        let request = get_request(Some("http://192.168.1.100:9117/"), "testtoken".to_owned(), HashMap::from([("t", "movie".to_owned())]));

        assert_eq!(request.url, "http://192.168.1.100:9117/api/v2.0/indexers/all/results?apikey=testtoken&t=movie");
    }
}
