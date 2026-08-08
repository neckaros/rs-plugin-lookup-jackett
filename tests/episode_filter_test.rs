#[path = "../src/episode_filter.rs"]
mod episode_filter;

use episode_filter::{title_matches_episode, title_matches_season};

#[test]
fn accepts_requested_episode_and_season_pack() {
    assert!(title_matches_episode(
        "From.S01E01.MULTI.1080p.WEB-DL",
        1,
        1
    ));
    assert!(title_matches_episode(
        "From.S01.MULTI.VFI.2160p.WEB.EAC3.5.1.HEVC-HYPERION",
        1,
        1
    ));
    assert!(title_matches_episode(
        "From.2022.S01.VFF.1080p.WEBRip.EAC3.5.1.AV1-MonoDiSC",
        1,
        1
    ));
}

#[test]
fn rejects_other_seasons_and_episodes() {
    assert!(!title_matches_episode(
        "From.S04E01.MULTI.VFF.1080p.WEB",
        1,
        1
    ));
    assert!(!title_matches_episode(
        "From.S03.PROPER.MULTI.2160p.WEB",
        1,
        1
    ));
    assert!(!title_matches_episode("From.S01E02.1080p.WEB", 1, 1));
}

#[test]
fn accepts_multi_season_pack_when_it_includes_the_requested_season() {
    let title = "From.S01-S03.COMPLETE.1080p.WEB";

    assert!(title_matches_episode(title, 1, 1));
    assert!(title_matches_episode(title, 2, 1));
    assert!(title_matches_episode(title, 3, 1));
    assert!(!title_matches_episode(title, 4, 1));
    assert!(!title_matches_episode(
        "From.S03-S04.COMPLETE.1080p.WEB",
        1,
        1
    ));
}

#[test]
fn supports_common_release_names() {
    assert!(title_matches_episode("Show.S1E1.720p", 1, 1));
    assert!(title_matches_episode("Show.S01.E01.720p", 1, 1));
    assert!(title_matches_episode("Show - 1x01 - Pilot", 1, 1));
    assert!(title_matches_episode("Show.S01E01-E02.1080p", 1, 2));
    assert!(title_matches_episode("Show.S01.DTS5.1.1080p", 1, 1));
    assert!(!title_matches_episode("Show.S11E01.720p", 1, 1));
    assert!(!title_matches_episode("Show.S01E10.720p", 1, 1));
}

#[test]
fn season_lookup_accepts_episodes_and_packs_only_for_that_season() {
    assert!(title_matches_season("From.S01E07.1080p.WEB", 1));
    assert!(title_matches_season("From.S01.COMPLETE.2160p.WEB", 1));
    assert!(title_matches_season("From.S01-S03.COMPLETE.1080p.WEB", 2));
    assert!(title_matches_season("From - 1x07 - Episode", 1));
    assert!(!title_matches_season("From.S03E01.1080p.WEB", 1));
    assert!(!title_matches_season("From.S03-S04.COMPLETE.1080p.WEB", 1));
}
