//! The chunk reader must yield exactly the stream's lines (respecting limits) for any input
//! and any chunk/line size.
#![no_main]

use libfuzzer_sys::fuzz_target;
use octopuson_logparse::input::{ChunkReader, LineReader};

fuzz_target!(|input: (u8, u8, &[u8])| {
    let (chunk_size, max_line, bytes) = input;
    let chunk_size = usize::from(chunk_size).max(1);
    let max_line = usize::from(max_line).max(1);

    let mut chunks = ChunkReader::with_sizes(bytes, chunk_size, max_line);
    let mut numbers = Vec::new();
    while let Ok(Some(chunk)) = chunks.next_chunk() {
        for line in chunk.lines() {
            assert!(line.bytes.len() <= max_line);
            numbers.push(line.number);
        }
    }
    assert!(numbers.windows(2).all(|pair| pair[1] == pair[0] + 1));

    // Lines that fit in a chunk must match the simple line reader exactly.
    if bytes.split(|&b| b == b'\n').all(|line| line.len() < chunk_size) {
        let mut lines = LineReader::with_max_line_bytes(bytes, max_line);
        let mut expected = 0_usize;
        while let Ok(Some(_)) = lines.next_line() {
            expected += 1;
        }
        assert_eq!(numbers.len(), expected);
    }
});
