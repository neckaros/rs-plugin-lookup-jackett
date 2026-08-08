fn parse_number(bytes: &[u8], start: usize) -> Option<(u32, usize)> {
    let mut end = start;
    let mut value = 0u32;

    while let Some(byte) = bytes.get(end).filter(|byte| byte.is_ascii_digit()) {
        value = value.checked_mul(10)?.checked_add((*byte - b'0') as u32)?;
        end += 1;
    }

    (end > start).then_some((value, end))
}

fn is_episode_separator(byte: u8) -> bool {
    matches!(byte, b'.' | b'-' | b'_' | b' ')
}

/// Match any release that belongs to the requested season, including individual
/// episodes, single-season packs and ranges such as S01-S03.
pub(crate) fn title_matches_season(title: &str, season: u32) -> bool {
    let bytes = title.as_bytes();

    for start in 0..bytes.len() {
        if bytes[start].eq_ignore_ascii_case(&b's')
            && (start == 0 || !bytes[start - 1].is_ascii_alphanumeric())
        {
            let Some((range_start, mut cursor)) = parse_number(bytes, start + 1) else {
                continue;
            };
            if range_start == season {
                return true;
            }

            let season_number_end = cursor;
            while bytes
                .get(cursor)
                .is_some_and(|byte| is_episode_separator(*byte))
            {
                cursor += 1;
            }

            let is_range = bytes[season_number_end..cursor].contains(&b'-')
                && bytes
                    .get(cursor)
                    .is_some_and(|byte| byte.eq_ignore_ascii_case(&b's'));
            if is_range {
                if let Some((range_end, _)) = parse_number(bytes, cursor + 1) {
                    if season >= range_start.min(range_end) && season <= range_start.max(range_end)
                    {
                        return true;
                    }
                }
            }
        }

        // Alternative episode naming: 1x02.
        if start == 0 || !bytes[start - 1].is_ascii_alphanumeric() {
            if let Some((title_season, cursor)) = parse_number(bytes, start) {
                if title_season == season
                    && title_season <= 100
                    && bytes
                        .get(cursor)
                        .is_some_and(|byte| byte.eq_ignore_ascii_case(&b'x'))
                    && parse_number(bytes, cursor + 1).is_some()
                {
                    return true;
                }
            }
        }
    }

    false
}

/// Jackett indexers do not consistently honor the Season/Ep parameters. Check the
/// release name as well so an IMDb match for another season is not returned.
pub(crate) fn title_matches_episode(title: &str, season: u32, episode: u32) -> bool {
    let bytes = title.as_bytes();
    let mut has_matching_episode = false;
    let mut has_matching_season_pack = false;

    for start in 0..bytes.len() {
        // S01E02, S01.E02, S01 E02 and multi-episode releases such as
        // S01E01E02 or S01E01-E02.
        if bytes[start].eq_ignore_ascii_case(&b's')
            && (start == 0 || !bytes[start - 1].is_ascii_alphanumeric())
        {
            let Some((title_season, mut cursor)) = parse_number(bytes, start + 1) else {
                continue;
            };

            let season_number_end = cursor;
            while bytes
                .get(cursor)
                .is_some_and(|byte| is_episode_separator(*byte))
            {
                cursor += 1;
            }

            // Multi-season packs commonly use S01-S03. Accept the pack when
            // the requested season falls anywhere inside that inclusive range.
            let is_range = bytes[season_number_end..cursor].contains(&b'-')
                && bytes
                    .get(cursor)
                    .is_some_and(|byte| byte.eq_ignore_ascii_case(&b's'));
            if is_range {
                if let Some((range_end, mut range_cursor)) = parse_number(bytes, cursor + 1) {
                    while bytes
                        .get(range_cursor)
                        .is_some_and(|byte| is_episode_separator(*byte))
                    {
                        range_cursor += 1;
                    }

                    let range_has_episode = bytes
                        .get(range_cursor)
                        .is_some_and(|byte| byte.eq_ignore_ascii_case(&b'e'));
                    if !range_has_episode
                        && season >= title_season.min(range_end)
                        && season <= title_season.max(range_end)
                    {
                        has_matching_season_pack = true;
                    }
                }
                continue;
            }

            if bytes
                .get(cursor)
                .is_some_and(|byte| byte.eq_ignore_ascii_case(&b'e'))
            {
                if title_season != season {
                    continue;
                }

                cursor += 1;
                loop {
                    let Some((title_episode, end)) = parse_number(bytes, cursor) else {
                        break;
                    };
                    if title_episode == episode {
                        has_matching_episode = true;
                    }

                    cursor = end;
                    while bytes
                        .get(cursor)
                        .is_some_and(|byte| is_episode_separator(*byte))
                    {
                        cursor += 1;
                    }
                    if !bytes
                        .get(cursor)
                        .is_some_and(|byte| byte.eq_ignore_ascii_case(&b'e'))
                    {
                        break;
                    }
                    cursor += 1;
                }
            } else if title_season == season {
                has_matching_season_pack = true;
            }
        }

        // Alternative release naming: 1x02.
        if start == 0 || !bytes[start - 1].is_ascii_alphanumeric() {
            if let Some((title_season, cursor)) = parse_number(bytes, start) {
                if title_season <= 100
                    && bytes
                        .get(cursor)
                        .is_some_and(|byte| byte.eq_ignore_ascii_case(&b'x'))
                {
                    if let Some((title_episode, end)) = parse_number(bytes, cursor + 1) {
                        if bytes.get(end).is_none_or(|byte| !byte.is_ascii_digit()) {
                            if title_season == season && title_episode == episode {
                                has_matching_episode = true;
                            }
                        }
                    }
                }
            }
        }
    }

    has_matching_episode || has_matching_season_pack
}
