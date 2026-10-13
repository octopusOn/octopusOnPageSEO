//! Single-thread parse throughput per format.

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use seonexus_logparse::FormatRegistry;

const LINES: &[(&str, &str)] = &[
    (
        "traefik-clf",
        r#"57.141.20.59 - - [07/Oct/2026:18:39:53 +0000] "GET /_next/image?url=%2Fflags%2Fgb.svg&w=828&q=75 HTTP/2.0" 200 535 "-" "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 (compatible; meta-externalagent/1.1 (+https://developers.facebook.com/docs/sharing/webmasters/crawler))" 12771189 "https-0-u10hd94shx9vwxprkmfyq0yw@docker" "http://172.18.0.6:3000" 5ms"#,
    ),
    (
        "combined",
        r#"66.249.66.1 - - [07/Oct/2026:18:39:53 +0300] "GET /urun/kirmizi-ayakkabi?renk=kirmizi HTTP/1.1" 200 15342 "-" "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)""#,
    ),
    (
        "traefik-json",
        r#"{"ClientAddr":"66.249.66.1:52344","ClientHost":"66.249.66.1","DownstreamContentSize":68,"DownstreamStatus":200,"Duration":812345,"RequestCount":42,"RequestHost":"example.com","RequestMethod":"GET","RequestPath":"/robots.txt","RequestProtocol":"HTTP/1.1","RouterName":"web@docker","ServiceURL":"http://172.18.0.6:3000","StartUTC":"2026-10-07T18:39:53.123456789Z","request_User-Agent":"Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)","level":"info"}"#,
    ),
];

fn parse_benchmarks(c: &mut Criterion) {
    let registry = FormatRegistry::builtin();
    let mut group = c.benchmark_group("parse_line");
    group.throughput(Throughput::Elements(1));
    for (id, line) in LINES {
        let Some(format) = registry.get(id) else {
            continue;
        };
        group.bench_function(*id, |b| b.iter(|| format.parse(black_box(line))));
    }
    group.finish();
}

criterion_group!(benches, parse_benchmarks);
criterion_main!(benches);
