//! Built-in format presets, expressed as templates.

use super::TemplateFormat;
use super::compiled::Token;
use super::error::TemplateError;
use super::spec::{DurationUnit, FieldSpec};

fn lit(text: &str) -> Token {
    Token::literal(text)
}

fn field(spec: FieldSpec) -> Token {
    Token::field(spec, spec.name())
}

/// `%h %l %u %t "%r" %>s %b`
fn common_tokens() -> Vec<Token> {
    vec![
        field(FieldSpec::ClientIp),
        lit(" "),
        field(FieldSpec::Ignore),
        lit(" "),
        field(FieldSpec::Ignore),
        lit(" ["),
        field(FieldSpec::TimeClf),
        lit("] \""),
        field(FieldSpec::RequestLine),
        lit("\" "),
        field(FieldSpec::Status),
        lit(" "),
        field(FieldSpec::Bytes),
    ]
}

/// Common + ` "%{Referer}i" "%{User-Agent}i"`
fn combined_tokens() -> Vec<Token> {
    let mut tokens = common_tokens();
    tokens.extend([
        lit(" \""),
        field(FieldSpec::Referer),
        lit("\" \""),
        field(FieldSpec::UserAgent),
        lit("\""),
    ]);
    tokens
}

/// Combined + Traefik's ` <count> "<router>" "<server URL>" <duration>ms`.
fn traefik_clf_tokens() -> Vec<Token> {
    let mut tokens = combined_tokens();
    tokens.extend([
        lit(" "),
        field(FieldSpec::RequestSeq),
        lit(" \""),
        field(FieldSpec::Router),
        lit("\" \""),
        field(FieldSpec::Upstream),
        lit("\" "),
        field(FieldSpec::Duration(DurationUnit::Millis)),
        lit("ms"),
    ]);
    tokens
}

/// Traefik access log in its default Common Log Format variant.
pub(crate) fn traefik_clf() -> Result<TemplateFormat, TemplateError> {
    TemplateFormat::compile("traefik-clf", "Traefik (CLF)", traefik_clf_tokens())
}

/// NCSA combined format, the default of Nginx and a standard Apache nickname.
pub(crate) fn combined() -> Result<TemplateFormat, TemplateError> {
    TemplateFormat::compile("combined", "Nginx / Apache combined", combined_tokens())
}

/// NCSA common format (Apache `common`).
pub(crate) fn common() -> Result<TemplateFormat, TemplateError> {
    TemplateFormat::compile("common", "Apache common", common_tokens())
}
