//! Country and language codes the Google APIs want: ISO 3166-1 alpha-3 (Search Console),
//! geo target constants (2000 + ISO numeric) and language constants (Google Ads).

/// `(alpha-2, alpha-3, numeric)` of the countries agencies work with most.
const COUNTRIES: &[(&str, &str, u16)] = &[
    ("tr", "tur", 792),
    ("az", "aze", 31),
    ("cy", "cyp", 196),
    ("us", "usa", 840),
    ("gb", "gbr", 826),
    ("ie", "irl", 372),
    ("ca", "can", 124),
    ("au", "aus", 36),
    ("nz", "nzl", 554),
    ("de", "deu", 276),
    ("at", "aut", 40),
    ("ch", "che", 756),
    ("nl", "nld", 528),
    ("be", "bel", 56),
    ("fr", "fra", 250),
    ("es", "esp", 724),
    ("pt", "prt", 620),
    ("it", "ita", 380),
    ("gr", "grc", 300),
    ("bg", "bgr", 100),
    ("ro", "rou", 642),
    ("pl", "pol", 616),
    ("cz", "cze", 203),
    ("hu", "hun", 348),
    ("se", "swe", 752),
    ("no", "nor", 578),
    ("dk", "dnk", 208),
    ("fi", "fin", 246),
    ("ru", "rus", 643),
    ("ua", "ukr", 804),
    ("by", "blr", 112),
    ("kz", "kaz", 398),
    ("uz", "uzb", 860),
    ("ge", "geo", 268),
    ("sa", "sau", 682),
    ("ae", "are", 784),
    ("qa", "qat", 634),
    ("kw", "kwt", 414),
    ("iq", "irq", 368),
    ("eg", "egy", 818),
    ("ma", "mar", 504),
    ("il", "isr", 376),
    ("in", "ind", 356),
    ("br", "bra", 76),
    ("mx", "mex", 484),
    ("jp", "jpn", 392),
    ("kr", "kor", 410),
    ("za", "zaf", 710),
];

/// Google Ads language constants by ISO 639-1 code.
const LANGUAGES: &[(&str, u16)] = &[
    ("en", 1000),
    ("de", 1001),
    ("fr", 1002),
    ("es", 1003),
    ("it", 1004),
    ("ja", 1005),
    ("nl", 1010),
    ("pt", 1014),
    ("ar", 1019),
    ("pl", 1030),
    ("ru", 1031),
    ("tr", 1037),
];

/// ISO alpha-3 code (`tr` → `tur`).
pub fn alpha3(country: &str) -> Option<&'static str> {
    COUNTRIES
        .iter()
        .find(|(code, _, _)| *code == country)
        .map(|(_, alpha3, _)| *alpha3)
}

/// Google Ads geo target constant of a country (`tr` → 2792).
pub fn geo_target(country: &str) -> Option<u32> {
    COUNTRIES
        .iter()
        .find(|(code, _, _)| *code == country)
        .map(|(_, _, numeric)| 2_000 + u32::from(*numeric))
}

/// Google Ads language constant (`tr` → 1037).
pub fn language_constant(language: &str) -> Option<u16> {
    LANGUAGES
        .iter()
        .find(|(code, _)| *code == language)
        .map(|(_, id)| *id)
}
