//! The Tranco list: a research ranking of the top million sites, refreshed daily.
//!
//! The list combines several popularity sources, some licensed for non-commercial use only
//! (Cloudflare Radar, CC BY-NC 4.0), so the program never downloads it unasked: the user
//! switches it on after reading the notice.

/// Download address of the latest list (zip with `top-1m.csv`).
pub const LIST_URL: &str = "https://tranco-list.eu/top-1m.csv.zip";

/// Address that returns the id of the latest list.
pub const LIST_ID_URL: &str = "https://tranco-list.eu/top-1m-id";

/// Licence notice shown before the first download.
pub const NOTICE: &str = "The Tranco list (tranco-list.eu, KU Leuven) aggregates Cisco Umbrella, Majestic (CC BY 3.0), Chrome UX Report (CC BY-SA 4.0), Cloudflare Radar (CC BY-NC 4.0) and Farsight data. Cloudflare Radar data may only be used non-commercially. Cite: Le Pochat et al., NDSS 2019.";

/// Rows of the list: `rank,domain` lines; malformed lines are skipped.
pub fn rows(csv: &str) -> impl Iterator<Item = (u32, &str)> {
    csv.lines().filter_map(|line| {
        let (rank, domain) = line.trim().split_once(',')?;
        let rank = rank.trim().parse().ok()?;
        let domain = domain.trim();
        (!domain.is_empty()).then_some((rank, domain))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_rows() {
        let rows: Vec<_> = rows("1,google.com\n2,facebook.com\nbad\n3,\n").collect();
        assert_eq!(rows, [(1, "google.com"), (2, "facebook.com")]);
    }
}
