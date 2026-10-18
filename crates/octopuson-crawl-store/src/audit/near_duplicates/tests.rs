use super::*;

fn group(id: u32, hash: u64, words: u32) -> Group {
    Group {
        pages: vec![id],
        text_hash: u64::from(id) + 1,
        simhash: hash,
        words,
    }
}

#[test]
fn radius_search_matches_exhaustive_existence_with_length_and_exact_hash_guards() {
    let mut state = 0xdabc_1a2b_1234_5678u64;
    let mut groups = Vec::new();
    for id in 0..120 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        groups.push(group(id * 2, state, 200 + id % 80));
        // Flips in separate bands ensure candidates are found through the remaining band.
        groups.push(group(
            id * 2 + 1,
            state ^ (1 << (id % 16)) ^ (1 << (16 + id % 16)) ^ (1 << (32 + id % 16)),
            250 + id % 80,
        ));
    }
    groups[3].text_hash = groups[2].text_hash;
    let result = witnesses(&groups);
    for (i, found) in result.iter().enumerate() {
        let valid = |j: usize| {
            i != j
                && groups[i].text_hash != groups[j].text_hash
                && hamming_distance(groups[i].simhash, groups[j].simhash) <= 3
                && u64::from(groups[i].words.min(groups[j].words)) * 5
                    >= u64::from(groups[i].words.max(groups[j].words)) * 4
        };
        assert_eq!(found.is_some(), (0..groups.len()).any(valid), "group {i}");
        if let Some((j, distance)) = found {
            assert!(valid(*j));
            assert_eq!(
                *distance,
                hamming_distance(groups[i].simhash, groups[*j].simhash)
            );
        }
    }
}

#[test]
fn accepts_three_bits_and_eighty_percent_but_rejects_four_bits_and_shorter_text() {
    let a = 0x1234_5678_abcd_ef01;
    assert!(
        witnesses(&[group(0, a, 200), group(1, a ^ 0x7, 250)])
            .iter()
            .all(Option::is_some)
    );
    assert!(
        witnesses(&[group(0, a, 200), group(1, a ^ 0xf, 200)])
            .iter()
            .all(Option::is_none)
    );
    assert!(
        witnesses(&[group(0, a, 200), group(1, a ^ 0x7, 251)])
            .iter()
            .all(Option::is_none)
    );
    // A shared fingerprint is still a candidate when normalized exact text differs.
    assert!(
        witnesses(&[group(0, 0, 200), group(1, 0, 200)])
            .iter()
            .all(Option::is_some)
    );
}

#[test]
fn dense_fingerprints_store_one_witness_per_group_instead_of_all_pairs() {
    let groups: Vec<_> = (0..5000).map(|id| group(id, u64::MAX, 300)).collect();
    let matches = witnesses(&groups);
    assert_eq!(matches.len(), 5000);
    for (i, found) in matches.into_iter().enumerate() {
        let (j, distance) = found.unwrap();
        assert_ne!(i, j);
        assert_eq!(distance, 0);
    }
}
