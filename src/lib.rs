use std::collections::HashMap;

use extism_pdk::{http, plugin_fn, FnResult, HttpRequest, Json, WithReturnCode};
use rs_plugin_common_interfaces::{CredentialType, PluginInformation, PluginType};
use rs_plugin_lookup_interfaces::{RsLookupQuery, RsLookupResult, RsLookupWrapper};
use serde::{Deserialize, Serialize};


#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")] 
pub struct JackettResult {
    pub title: String,
    pub magnet_uri: String,
    pub seeders: u64,
    pub tmdb: Option<u64>,
    pub imdb: Option<String>
}
#[plugin_fn]
pub fn infos() -> FnResult<Json<PluginInformation>> {
    Ok(Json(
        PluginInformation { name: "jackett_lookup".into(), kind: PluginType::Lookup, version: 1, publisher: "neckaros".into(), description: "fetch possible movies or episode with the Jackett API".into(), credential_kind: Some(CredentialType::Token), ..Default::default() }
    ))
}

pub fn get_request(url: Option<&str>, token: String, params: HashMap<&str, String>) -> HttpRequest {
    let url = url.unwrap_or("http://127.0.0.1:9117");
    let params_string = params.into_iter().map(|(key, value)| format!("{}={}", key, value)).collect::<Vec<_>>().join("&");
    HttpRequest {
        url: format!("{}?apikey={}&{}", url, token, params_string),
        headers: Default::default(),
        method: Some("GET".into()),
    }
}

pub fn process(Json(lookup): Json<RsLookupWrapper>) -> FnResult<Json<RsLookupResult>> {
    if let Some(token) = lookup.credential.and_then(|l| l.password) {
        if let RsLookupQuery::Episode(episode_query) = lookup.query {
            let mut params = HashMap::from([("t", "tvsearch".to_owned()),("q", episode_query.serie), ("season", episode_query.season.to_string())]);
            if let Some(episode) = episode_query.number {
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
            }
            let request = get_request(None, token, params);

            let res = http::request::<()>(&request, None);
            if let Ok(res) = res {
                let r: Vec<JackettResult> = res.json()?;
                println!("result {:?}", r);
                Ok(Json(RsLookupResult::NotFound))
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
                let r: Vec<JackettResult> = res.json()?;
                println!("result {:?}", r);
                Ok(Json(RsLookupResult::NotFound))
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
    use super::*;

    #[test]
    fn it_works() {
        
    }
}
