use std::collections::HashMap;

use extism_pdk::{http, info, plugin_fn, FnResult, HttpRequest, Json, WithReturnCode};
use plugin_request_interfaces::{RsRequest, RsRequestStatus};
use rs_plugin_common_interfaces::{CredentialType, PluginInformation, PluginType};
use rs_plugin_lookup_interfaces::{RsLookupQuery, RsLookupResult, RsLookupWrapper};
use serde::{Deserialize, Serialize};
use urlencoding::encode;


#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "PascalCase")] 
pub struct JackettResults {
    pub results: Vec<JackettResult>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "PascalCase")] 
pub struct JackettResult {
    pub title: String,
    pub link: Option<String>,
    pub magnet_uri: Option<String>,
    pub seeders: u64,
    pub tmdb: Option<u64>,
    pub imdb: Option<String>
}

impl From<JackettResult> for RsRequest {
    fn from(value: JackettResult) -> Self {
        RsRequest { upload_id: None, url: value.magnet_uri.or(value.link).unwrap_or("".to_owned()), mime: Some("applications/x-bittorrent".to_owned()), size: None, filename: Some(value.title), status: RsRequestStatus::Unprocessed, ..Default::default() }
    }
}


#[plugin_fn]
pub fn infos() -> FnResult<Json<PluginInformation>> {
    Ok(Json(
        PluginInformation { name: "jackett_lookup".into(), kind: PluginType::Lookup, version: 1, publisher: "neckaros".into(), description: "fetch possible movies or episode with the Jackett API".into(), credential_kind: Some(CredentialType::Token), ..Default::default() }
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
pub fn process(Json(lookup): Json<RsLookupWrapper>) -> FnResult<Json<RsLookupResult>> {
    if let Some(token) = lookup.credential.and_then(|l| l.password) {
        if let RsLookupQuery::Episode(episode_query) = lookup.query {
            let q =  if let Some(number) = episode_query.number {
                format!("{} s{:02}e{:02}", episode_query.serie, episode_query.season, number)
            } else {
                format!("{} s{:02}", episode_query.serie, episode_query.season)
            };
            let params = HashMap::from([("t", "tvsearch".to_owned()),("Query", q)]);
            /*if let Some(episode) = episode_query.number {
                params.insert("ep", episode.to_string());
            }
            
            if let Some(tmdb) = episode_query.tmdb {
                params.insert("tmdbid", tmdb.to_string());
            }
            if let Some(imdb) = episode_query.imdb {
                params.insert("imdbid", imdb);
            }
            if let Some(trakt) = episode_query.trakt {
                params.insert("traktid", trakt.to_string());
            }
            if let Some(tvdb) = episode_query.tvdb {
                params.insert("tvdbid", tvdb.to_string());
            }*/
            let request = get_request(None, token, params);

            let res = http::request::<()>(&request, None);
            if let Ok(res) = res {
                let r: JackettResults = res.json()?;
                let requests: Vec<RsRequest> = r.results.into_iter().map(|t| RsRequest::from(t)).collect();
                Ok(Json(RsLookupResult::Requests(requests)))
            } else {
                Err(WithReturnCode(res.err().unwrap(), 500))
            }
        } else if let RsLookupQuery::Movie(movie_query) = lookup.query {
            let mut params = HashMap::from([("t", "movie".to_owned()), ("q", movie_query.name)]);
            if let Some(tmdb) = movie_query.tmdb {
                params.insert("tmdbid", tmdb.to_string());
            }
            if let Some(imdb) = movie_query.imdb {
                params.insert("imdbid", imdb);
            }
            if let Some(trakt) = movie_query.trakt {
                params.insert("traktid", trakt.to_string());
            }
            let request = get_request(None, token, params);

            let res = http::request::<()>(&request, None);
            if let Ok(res) = res {
                let r: JackettResults = res.json()?;
                let requests: Vec<RsRequest> = r.results.into_iter().map(|t| RsRequest::from(t)).collect();
                Ok(Json(RsLookupResult::Requests(requests)))
            } else {
                Err(WithReturnCode(res.err().unwrap(), 500))
            }
        } else {
            Ok(Json(RsLookupResult::NotApplicable))
        }
        
    } else {
        Err(WithReturnCode::new(extism_pdk::Error::msg("Need token"), 401))
    }
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::wasm_bindgen_test;

    use super::*;

    #[wasm_bindgen_test]
    fn request() {
        let request = get_request(None, "testtoken".to_owned(), HashMap::from([("testparam1", "A".to_owned()), ("testparam2", "B".to_owned())]));

        assert_eq!(request.url, "http://127.0.0.1:9117/?apikey=testtoken&testparam1=A&testparam2=B");
    }
}
