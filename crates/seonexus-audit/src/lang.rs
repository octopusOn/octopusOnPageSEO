//! Conservative validation of language/region hreflang identifiers.

const LANGUAGES: &str = "aa ab ae af ak am an ar as av ay az ba be bg bh bi bm bn bo br bs ca ce ch co cr cs cu cv cy da de dv dz ee el en eo es et eu fa ff fi fj fo fr fy ga gd gl gn gu gv ha he hi ho hr ht hu hy hz ia id ie ig ii ik io is it iu ja jv ka kg ki kj kk kl km kn ko kr ks ku kv kw ky la lb lg li ln lo lt lu lv mg mh mi mk ml mn mr ms mt my na nb nd ne ng nl nn no nr nv ny oc oj om or os pa pi pl ps pt qu rm rn ro ru rw sa sc sd se sg si sk sl sm sn so sq sr ss st su sv sw ta te tg th ti tk tl tn to tr ts tt tw ty ug uk ur uz ve vi vo wa wo xh yi yo za zh zu";

// ISO 3166-1 alpha-2 (IANA time-zone country table). UK/EU/UN are reserved, not regions.
const REGIONS: &str = "ad ae af ag ai al am ao aq ar as at au aw ax az ba bb bd be bf bg bh bi bj bl bm bn bo bq br bs bt bv bw by bz ca cc cd cf cg ch ci ck cl cm cn co cr cu cv cw cx cy cz de dj dk dm do dz ec ee eg eh er es et fi fj fk fm fo fr ga gb gd ge gf gg gh gi gl gm gn gp gq gr gs gt gu gw gy hk hm hn hr ht hu id ie il im in io iq ir is it je jm jo jp ke kg kh ki km kn kp kr kw ky kz la lb lc li lk lr ls lt lu lv ly ma mc md me mf mg mh mk ml mm mn mo mp mq mr ms mt mu mv mw mx my mz na nc ne nf ng ni nl no np nr nu nz om pa pe pf pg ph pk pl pm pn pr ps pt pw py qa re ro rs ru rw sa sb sc sd se sg sh si sj sk sl sm sn so sr ss st sv sx sy sz tc td tf tg th tj tk tl tm tn to tr tt tv tw tz ua ug um us uy uz va vc ve vg vi vn vu wf ws ye yt za zm zw";

fn valid_region(value: &str) -> bool {
    REGIONS.split_whitespace().any(|region| region == value)
}

/// Accepts `x-default` and ISO language codes with an optional region or script.
/// Unknown language codes and locale underscores are rejected.
pub fn is_valid_hreflang(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    if value == "x-default" {
        return true;
    }
    let mut parts = value.split('-');
    let language = parts.next().unwrap_or_default();
    if !LANGUAGES.split_whitespace().any(|code| code == language) {
        return false;
    }
    let rest: Vec<&str> = parts.collect();
    match rest.as_slice() {
        [] => true,
        [region] => {
            valid_region(region)
                || (region.len() == 4 && region.bytes().all(|b| b.is_ascii_alphabetic()))
        }
        [script, region] => {
            script.len() == 4
                && script.bytes().all(|b| b.is_ascii_alphabetic())
                && valid_region(region)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::is_valid_hreflang;

    #[test]
    fn accepts_languages_and_regions() {
        for code in ["tr", "en-GB", "zh-Hant-TW", "x-default"] {
            assert!(is_valid_hreflang(code), "{code}");
        }
        for code in [
            "",
            "en_UK",
            "zz-US",
            "en-UK",
            "en-EU",
            "es-419",
            "xx",
            "english",
            "en-gb-extra",
        ] {
            assert!(!is_valid_hreflang(code), "{code}");
        }
    }
}
