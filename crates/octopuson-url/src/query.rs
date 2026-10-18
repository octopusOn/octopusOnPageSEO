//! Query-string handling.

/// Parameters added by analytics and ad platforms; they never change the returned page.
const TRACKING_PREFIXES: &[&str] = &["utm_", "pk_", "mtm_"];
const TRACKING_NAMES: &[&str] = &[
    "gclid",
    "gclsrc",
    "gbraid",
    "wbraid",
    "dclid",
    "fbclid",
    "msclkid",
    "yclid",
    "twclid",
    "ttclid",
    "li_fat_id",
    "igshid",
    "mc_cid",
    "mc_eid",
    "_ga",
    "_gl",
    "_hsenc",
    "_hsmi",
    "srsltid",
    "ref_src",
];

/// Whether a query parameter name is a well-known tracking parameter (case-insensitive).
pub fn is_tracking_parameter(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    TRACKING_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
        || TRACKING_NAMES.contains(&name.as_str())
}

/// Removes the query parameters whose name `remove` matches from a request target such as
/// `/p?sid=1&x=2`, keeping the others in order; `None` when nothing was removed.
pub fn strip_parameters(target: &str, remove: impl Fn(&str) -> bool) -> Option<String> {
    let (path, query) = target.split_once('?')?;
    let mut removed = false;
    let kept = filter_parameters(query, |name| {
        let drop = remove(name);
        removed |= drop;
        !drop
    });
    if !removed {
        return None;
    }
    Some(if kept.is_empty() {
        path.to_owned()
    } else {
        format!("{path}?{kept}")
    })
}

/// Keeps the parameters `keep` accepts (by name), in their original order.
pub(crate) fn filter_parameters(query: &str, mut keep: impl FnMut(&str) -> bool) -> String {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .filter(|pair| keep(pair.split_once('=').map_or(*pair, |(name, _)| name)))
        .collect::<Vec<_>>()
        .join("&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_listed_parameters_only() {
        let sid = |name: &str| name.eq_ignore_ascii_case("sid");
        assert_eq!(
            strip_parameters("/p?sid=1&x=2", sid).as_deref(),
            Some("/p?x=2")
        );
        assert_eq!(strip_parameters("/p?SID=1", sid).as_deref(), Some("/p"));
        assert_eq!(strip_parameters("/p?x=2", sid), None);
        assert_eq!(strip_parameters("/p", sid), None);
        assert_eq!(
            strip_parameters("/?utm_source=a&id=3&gclid=x", is_tracking_parameter).as_deref(),
            Some("/?id=3")
        );
    }

    #[test]
    fn recognizes_tracking_parameters() {
        for name in [
            "utm_source",
            "UTM_Medium",
            "gclid",
            "fbclid",
            "_ga",
            "srsltid",
        ] {
            assert!(is_tracking_parameter(name), "{name}");
        }
        for name in ["page", "q", "utm", "id"] {
            assert!(!is_tracking_parameter(name), "{name}");
        }
    }

    #[test]
    fn filters_and_drops_empty_pairs() {
        let kept = filter_parameters("a=1&&utm_source=x&b&gclid=2", |n| !is_tracking_parameter(n));
        assert_eq!(kept, "a=1&b");
    }
}
