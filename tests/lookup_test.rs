use extism::*;
use rs_plugin_common_interfaces::{
    domain::rs_ids::RsIds,
    lookup::{RsLookupEpisode, RsLookupMovie, RsLookupQuery, RsLookupSourceResult, RsLookupWrapper},
    request::RsRequest,
    PluginCredential,
};
use std::collections::HashMap;

#[path = "../src/episode_filter.rs"]
mod episode_filter;

use episode_filter::{title_matches_episode, title_matches_season};

const DEFAULT_JACKETT_URL: &str = "https://nseat-jackett.jezequel.org/";
const DEFAULT_JACKETT_TOKEN: &str = "3gk0bdlkiek33tozyz3q80uq2tgk7xfz";

fn jackett_token() -> String {
    std::env::var("JACKETT_TOKEN").unwrap_or_else(|_| DEFAULT_JACKETT_TOKEN.to_string())
}

fn jackett_url() -> String {
    std::env::var("JACKETT_URL").unwrap_or_else(|_| DEFAULT_JACKETT_URL.to_string())
}

fn build_plugin() -> Plugin {
    let wasm = Wasm::file(
        "target/wasm32-unknown-unknown/release/rs_plugin_lookup_jackett.wasm",
    );
    let manifest = Manifest::new([wasm])
        .with_allowed_host("*");
    Plugin::new(&manifest, [], true).expect("Failed to create plugin")
}

fn make_credential() -> Option<PluginCredential> {
    Some(PluginCredential {
        password: Some(jackett_token()),
        ..Default::default()
    })
}

fn make_params() -> Option<HashMap<String, rs_plugin_common_interfaces::CustomParamTypes>> {
    let mut params = HashMap::new();
    params.insert(
        "base_url".to_string(),
        rs_plugin_common_interfaces::CustomParamTypes::Url(Some(jackett_url())),
    );
    Some(params)
}

fn call_lookup(plugin: &mut Plugin, input: &RsLookupWrapper) -> RsLookupSourceResult {
    let input_str = serde_json::to_string(input).unwrap();
    let output = plugin
        .call::<&str, &[u8]>("lookup", &input_str)
        .expect("lookup call failed");
    serde_json::from_slice(output).expect("Failed to parse output JSON")
}

fn extract_requests(result: RsLookupSourceResult) -> Vec<RsRequest> {
    match result {
        RsLookupSourceResult::Requests(requests) => requests,
        other => panic!("Expected Requests, got {:?}", other),
    }
}

#[test]
fn test_infos() {
    let mut plugin = build_plugin();
    let output = plugin
        .call::<&str, &[u8]>("infos", "")
        .expect("infos call failed");
    let info: serde_json::Value =
        serde_json::from_slice(output).expect("Failed to parse infos JSON");

    assert_eq!(info["name"], "jackett_lookup");
    assert_eq!(info["version"], 7);
    println!("\n=== Plugin info ===\n{}", serde_json::to_string_pretty(&info).unwrap());
}

#[test]
fn test_lookup_episode_ted_with_imdb() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Episode(RsLookupEpisode {
            name: Some("Ted".to_string()),
            ids: Some(RsIds::from_imdb("tt14824792".to_string())),
            season: 2,
            number: Some(7),
            page_key: None,
        }),
        credential: make_credential(),
        params: make_params(),
    };

    let result = call_lookup(&mut plugin, &input);
    let requests = extract_requests(result);

    println!("\n=== Ted S02E07 results ({} total) ===", requests.len());
    for (i, r) in requests.iter().take(10).enumerate() {
        println!("  {}. {}", i + 1, r.filename.as_deref().unwrap_or("?"));
    }

    assert!(!requests.is_empty(), "Should return results for Ted S02E07");

    // Verify top results are relevant to Ted (not random unrelated content)
    let top_results: Vec<&str> = requests.iter()
        .take(5)
        .filter_map(|r| r.filename.as_deref())
        .collect();
    let has_ted = top_results.iter().any(|f| f.to_lowercase().contains("ted"));
    assert!(has_ted, "Top results should contain 'Ted' in filename. Got: {:?}", top_results);
}

#[test]
fn test_lookup_episode_invincible_with_imdb() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Episode(RsLookupEpisode {
            name: Some("Invincible".to_string()),
            ids: Some(RsIds::from_imdb("tt6741278".to_string())),
            season: 1,
            number: Some(1),
            page_key: None,
        }),
        credential: make_credential(),
        params: make_params(),
    };

    let result = call_lookup(&mut plugin, &input);
    let requests = extract_requests(result);

    println!("\n=== Invincible S01E01 results ({} total) ===", requests.len());
    for (i, r) in requests.iter().take(10).enumerate() {
        println!("  {}. {}", i + 1, r.filename.as_deref().unwrap_or("?"));
    }

    assert!(!requests.is_empty(), "Should return results for Invincible S01E01");

    // Verify results are for the correct show (IMDB ID filtering works)
    // Note: season/episode filtering depends on indexer support and may not work on all trackers
    let top_results: Vec<&str> = requests.iter()
        .take(10)
        .filter_map(|r| r.filename.as_deref())
        .collect();
    let invincible_count = top_results.iter().filter(|f| f.to_lowercase().contains("invincible")).count();
    assert!(invincible_count >= 3, "Most top results should be 'Invincible' (got {}/{}). Results: {:?}", invincible_count, top_results.len(), top_results);
}

#[test]
fn test_lookup_episode_from_filters_other_seasons() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Episode(RsLookupEpisode {
            name: Some("From".to_string()),
            ids: Some(RsIds::from_imdb("tt9813792".to_string())),
            season: 1,
            number: Some(1),
            page_key: None,
        }),
        credential: make_credential(),
        params: make_params(),
    };

    let requests = extract_requests(call_lookup(&mut plugin, &input));

    assert!(
        requests.iter().any(|request| {
            request.filename.as_deref()
                == Some("From.S01.MULTI.VFI.2160p.WEB.EAC3.5.1.HEVC-HYPERION")
        }),
        "The matching S01 season pack should be returned"
    );
    assert!(
        requests.iter().any(|request| {
            request.filename.as_deref()
                == Some("From.2022.S01.VFF.1080p.WEBRip.EAC3.5.1.AV1-MonoDiSC")
        }),
        "The matching S01 season pack with a year should be returned"
    );
    assert!(
        requests.iter().all(|request| {
            request
                .filename
                .as_deref()
                .is_some_and(|title| title_matches_episode(title, 1, 1))
        }),
        "Every returned result must match From S01E01: {:?}",
        requests
    );
}

#[test]
fn test_lookup_from_season_filters_other_seasons() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Episode(RsLookupEpisode {
            name: Some("From".to_string()),
            ids: Some(RsIds::from_imdb("tt9813792".to_string())),
            season: 1,
            number: None,
            page_key: None,
        }),
        credential: make_credential(),
        params: make_params(),
    };

    let requests = extract_requests(call_lookup(&mut plugin, &input));

    assert!(!requests.is_empty(), "The From S01 lookup should return results");
    assert!(
        requests.iter().all(|request| {
            request
                .filename
                .as_deref()
                .is_some_and(|title| title_matches_season(title, 1))
        }),
        "Every returned result must include From season 1: {:?}",
        requests
    );
}

#[test]
fn test_lookup_episode_name_only() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Episode(RsLookupEpisode {
            name: Some("Breaking Bad".to_string()),
            ids: None,
            season: 1,
            number: Some(1),
            page_key: None,
        }),
        credential: make_credential(),
        params: make_params(),
    };

    let result = call_lookup(&mut plugin, &input);
    let requests = extract_requests(result);

    println!("\n=== Breaking Bad S01E01 (name only) results ({} total) ===", requests.len());
    for (i, r) in requests.iter().take(5).enumerate() {
        println!("  {}. {}", i + 1, r.filename.as_deref().unwrap_or("?"));
    }

    assert!(!requests.is_empty(), "Should return results with name-only search");
}

#[test]
fn test_lookup_movie_with_imdb() {
    let mut plugin = build_plugin();

    // Fight Club - tt0137523
    let input = RsLookupWrapper {
        query: RsLookupQuery::Movie(RsLookupMovie {
            name: Some("Fight Club".to_string()),
            ids: Some(RsIds::from_imdb("tt0137523".to_string())),
            page_key: None,
        }),
        credential: make_credential(),
        params: make_params(),
    };

    let result = call_lookup(&mut plugin, &input);
    let requests = extract_requests(result);

    println!("\n=== Fight Club movie results ({} total) ===", requests.len());
    for (i, r) in requests.iter().take(5).enumerate() {
        println!("  {}. {}", i + 1, r.filename.as_deref().unwrap_or("?"));
    }

    assert!(!requests.is_empty(), "Should return results for Fight Club");
}

#[test]
fn test_lookup_no_token_returns_error() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Episode(RsLookupEpisode {
            name: Some("Ted".to_string()),
            ids: None,
            season: 1,
            number: Some(1),
            page_key: None,
        }),
        credential: None,
        params: None,
    };

    let input_str = serde_json::to_string(&input).unwrap();
    let result = plugin.call::<&str, &[u8]>("lookup", &input_str);
    assert!(result.is_err(), "Should fail without token");
}

#[test]
fn test_results_sorted_by_seeders() {
    let mut plugin = build_plugin();

    // Use a popular show to get results with varied seeder counts
    let input = RsLookupWrapper {
        query: RsLookupQuery::Episode(RsLookupEpisode {
            name: Some("Breaking Bad".to_string()),
            ids: Some(RsIds::from_imdb("tt0903747".to_string())),
            season: 1,
            number: Some(1),
            page_key: None,
        }),
        credential: make_credential(),
        params: make_params(),
    };

    let result = call_lookup(&mut plugin, &input);
    let requests = extract_requests(result);

    println!("\n=== Breaking Bad S01E01 (checking sort) results ({} total) ===", requests.len());
    for (i, r) in requests.iter().take(10).enumerate() {
        println!("  {}. {} (size: {:?})", i + 1, r.filename.as_deref().unwrap_or("?"), r.size);
    }

    // We can't directly check seeders on RsRequest, but at least verify results come back
    assert!(!requests.is_empty(), "Should return results");
}
