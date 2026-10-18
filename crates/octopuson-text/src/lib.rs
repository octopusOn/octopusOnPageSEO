//! Text analysis for SEO.
//!
//! - [`Language`] and [`guess_language`]: Turkish, English, Russian or German, from stop word
//!   ratios and script.
//! - [`lower`] and [`fold`]: case folding with Turkish and Azerbaijani rules (`I` → `ı`,
//!   `İ` → `i`), and diacritic folding (`kahvé`, `çay` → `kahve`, `cay`) so words typed without
//!   Turkish letters still match.
//! - [`tokens`]: words, with suffixes after an apostrophe dropped (`Türkiye'nin` → `türkiye`).
//! - [`Analyzer`]: stop words, stems (Snowball, and [`turkish_key`] for Turkish) and n-grams; [`DocTerms`] counts the terms of a
//!   text, [`consensus`] finds the terms most top-ranking pages use and [`gaps`] those a page
//!   lacks.
//! - [`readability`]: Ateşman (Turkish) and Flesch reading ease (English).
//! - [`extract`]: visible text of an HTML page, its main content, paragraphs, headings, lists
//!   and tables.

mod analyzer;
mod case;
pub mod extract;
mod language;
pub mod readability;
mod stopwords;

pub use analyzer::{Analyzer, ConsensusTerm, DocTerms, Term, consensus, gaps, turkish_key};
pub use case::{fold, lower, tokens};
pub use extract::{PageText, extract};
pub use language::{Language, guess_language};
pub use readability::{Readability, readability};
pub use stopwords::is_stopword;
