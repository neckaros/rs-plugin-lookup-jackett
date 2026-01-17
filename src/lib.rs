use std::collections::HashMap;

use error::JackettError;
use extism_pdk::{http, info, log, plugin_fn, FnResult, HttpRequest, Json, LogLevel, WithReturnCode};

use rs_plugin_common_interfaces::{lookup::{RsLookupQuery, RsLookupSourceResult, RsLookupWrapper}, request::{RsRequest, RsRequestPluginRequest, RsRequestStatus}, CredentialType, PluginInformation, PluginType};
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
        PluginInformation { name: "jackett_lookup".into(), capabilities: vec![PluginType::Lookup, PluginType::Request], version: 1, interface_version: 1, publisher: "neckaros".into(), description: "fetch possible movies or episode with the Jackett API".into(), credential_kind: Some(CredentialType::Token), ..Default::default() }
    ))
}


pub fn get_request(url: Option<&str>, token: String, params: HashMap<&str, String>) -> HttpRequest {
    let url = url.unwrap_or("http://127.0.0.1:9117/api/v2.0/indexers/all/results");
    
    let params_string = params.into_iter().map(|(key, value)| format!("{}={}", key, encode(&value))).collect::<Vec<_>>().join("&");
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
        if let RsLookupQuery::Episode(episode_query) = lookup.query {
            let q =  if let Some(number) = episode_query.number {
                format!("{} s{:02}e{:02}", unidecode(&episode_query.serie), episode_query.season, number)
            } else {
                format!("{} s{:02}", unidecode(&episode_query.serie), episode_query.season)
            };
            let params = HashMap::from([("t", "tvsearch".to_owned()),("Query", q)]);

            let request = get_request(None, token.clone(), params);

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
            let params = HashMap::from([("t", "movie".to_owned()), ("Query", unidecode(&movie_query.name))]);
            
            let request = get_request(None, token.clone(), params);

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
                    let magnet = magnet_from_torrent(encoded);
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
                    let magnet = magnet_from_torrent(encoded);
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

        assert_eq!(request.url, "http://127.0.0.1:9117/?apikey=testtoken&testparam1=A&testparam2=B");
    }
}
