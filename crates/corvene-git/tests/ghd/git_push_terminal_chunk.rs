//! Port of GitHub Desktop's `app/test/unit/git/push-terminal-chunk-test.ts`.
//!
//! GitHub Desktop's `pushTerminalChunk` (`lib/git/push-terminal-chunk.ts`)
//! is `corvene_git::push_terminal_chunk`: GitHub Desktop's `git()`
//! (`lib/git/core.ts`) and `corvene_git::GitCommand` keep the last 256 KiB
//! of a command's combined stdout and stderr with it for the error a failed
//! command reports.
//!
//! - A chunk is a `Buffer` or a string in GitHub Desktop: here anything that
//!   is `AsRef<[u8]>`, a string literal for a string and a byte string
//!   (`b"…"`) for `Buffer.from(…)`.
//! - GitHub Desktop counts the capacity in JavaScript string length (UTF-16
//!   code units, so `'👋'` is 2) and its assertions measure the same way:
//!   [`js_length`] is JavaScript's `.length`. `chunks.join('')` is
//!   `chunks.concat()`.

use corvene_git::push_terminal_chunk;

/// JavaScript's `string.length`: the number of UTF-16 code units.
fn js_length(s: &str) -> usize {
    s.encode_utf16().count()
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › basic functionality › appends a string chunk to an empty buffer
#[test]
fn appends_a_string_chunk_to_an_empty_buffer() {
    let mut chunks: Vec<String> = Vec::new();
    push_terminal_chunk(&mut chunks, 100, "hello");
    assert_eq!(chunks, ["hello"]);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › basic functionality › appends multiple string chunks
#[test]
fn appends_multiple_string_chunks() {
    let mut chunks: Vec<String> = Vec::new();
    push_terminal_chunk(&mut chunks, 100, "hello");
    push_terminal_chunk(&mut chunks, 100, " world");
    assert_eq!(chunks, ["hello", " world"]);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › basic functionality › appends a Buffer chunk by converting it to string
#[test]
fn appends_a_buffer_chunk_by_converting_it_to_string() {
    let mut chunks: Vec<String> = Vec::new();
    push_terminal_chunk(&mut chunks, 100, b"hello");
    assert_eq!(chunks, ["hello"]);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › basic functionality › appends an empty string chunk
#[test]
fn appends_an_empty_string_chunk() {
    let mut chunks: Vec<String> = Vec::new();
    push_terminal_chunk(&mut chunks, 100, "");
    assert_eq!(chunks, [""]);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › basic functionality › appends an empty Buffer chunk
#[test]
fn appends_an_empty_buffer_chunk() {
    let mut chunks: Vec<String> = Vec::new();
    push_terminal_chunk(&mut chunks, 100, b"");
    assert_eq!(chunks, [""]);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › capacity management › does not trim when total length equals capacity
#[test]
fn does_not_trim_when_total_length_equals_capacity() {
    let mut chunks: Vec<String> = Vec::new();
    push_terminal_chunk(&mut chunks, 10, "0123456789");
    assert_eq!(chunks, ["0123456789"]);
    assert_eq!(js_length(&chunks.concat()), 10);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › capacity management › does not trim when total length is under capacity
#[test]
fn does_not_trim_when_total_length_is_under_capacity() {
    let mut chunks: Vec<String> = Vec::new();
    push_terminal_chunk(&mut chunks, 10, "12345");
    assert_eq!(chunks, ["12345"]);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › capacity management › removes entire first chunk when overrun exceeds first chunk length
#[test]
fn removes_entire_first_chunk_when_overrun_exceeds_first_chunk_length() {
    let mut chunks: Vec<String> = vec!["abc".into(), "def".into()];
    push_terminal_chunk(&mut chunks, 6, "ghij");
    // Total would be 10, capacity is 6, overrun is 4
    // First chunk 'abc' has length 3, so it's removed entirely
    // Remaining overrun is 1, so 'def' becomes 'ef'
    assert_eq!(chunks, ["ef", "ghij"]);
    assert_eq!(js_length(&chunks.concat()), 6);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › capacity management › partially trims first chunk when overrun is less than first chunk length
#[test]
fn partially_trims_first_chunk_when_overrun_is_less_than_first_chunk_length() {
    let mut chunks: Vec<String> = vec!["abcdef".into()];
    push_terminal_chunk(&mut chunks, 8, "ghi");
    // Total would be 9, capacity is 8, overrun is 1
    // First chunk 'abcdef' is trimmed by 1 character from the start
    assert_eq!(chunks, ["bcdef", "ghi"]);
    assert_eq!(js_length(&chunks.concat()), 8);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › capacity management › removes multiple chunks when necessary
#[test]
fn removes_multiple_chunks_when_necessary() {
    let mut chunks: Vec<String> = vec!["aa".into(), "bb".into(), "cc".into()];
    push_terminal_chunk(&mut chunks, 4, "dddd");
    // Total would be 10, capacity is 4, need to remove 6 characters
    // Remove 'aa' (2), remove 'bb' (2), remove 'cc' (2) = 6 removed
    assert_eq!(chunks, ["dddd"]);
    assert_eq!(js_length(&chunks.concat()), 4);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › capacity management › handles single chunk that exceeds capacity
#[test]
fn handles_single_chunk_that_exceeds_capacity() {
    let mut chunks: Vec<String> = Vec::new();
    push_terminal_chunk(&mut chunks, 5, "0123456789");
    // Chunk of 10 chars, capacity of 5
    // Should trim from the beginning to fit exactly 5 chars
    assert_eq!(chunks, ["56789"]);
    assert_eq!(js_length(&chunks.concat()), 5);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › capacity management › handles capacity of zero
#[test]
fn handles_capacity_of_zero() {
    let mut chunks: Vec<String> = Vec::new();
    push_terminal_chunk(&mut chunks, 0, "hello");
    // Everything should be trimmed, including the empty chunk
    assert_eq!(chunks, [] as [&str; 0]);
    assert_eq!(js_length(&chunks.concat()), 0);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › capacity management › handles capacity of one
#[test]
fn handles_capacity_of_one() {
    let mut chunks: Vec<String> = Vec::new();
    push_terminal_chunk(&mut chunks, 1, "hello");
    assert_eq!(chunks, ["o"]);
    assert_eq!(js_length(&chunks.concat()), 1);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › rolling buffer behavior › maintains rolling buffer with repeated pushes
#[test]
fn maintains_rolling_buffer_with_repeated_pushes() {
    let mut chunks: Vec<String> = Vec::new();
    let capacity = 15;

    push_terminal_chunk(&mut chunks, capacity, "aaaaa"); // 5 chars
    assert_eq!(js_length(&chunks.concat()), 5);

    push_terminal_chunk(&mut chunks, capacity, "bbbbb"); // 10 chars total
    assert_eq!(js_length(&chunks.concat()), 10);

    push_terminal_chunk(&mut chunks, capacity, "ccccc"); // 15 chars total
    assert_eq!(js_length(&chunks.concat()), 15);

    push_terminal_chunk(&mut chunks, capacity, "ddddd"); // would be 20, trimmed to 15
    assert_eq!(js_length(&chunks.concat()), 15);
    assert_eq!(chunks.concat(), "bbbbbcccccddddd");

    push_terminal_chunk(&mut chunks, capacity, "eeeee"); // would be 20, trimmed to 15
    assert_eq!(js_length(&chunks.concat()), 15);
    assert_eq!(chunks.concat(), "cccccdddddeeeee");
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › rolling buffer behavior › preserves newest content when trimming
#[test]
fn preserves_newest_content_when_trimming() {
    let mut chunks: Vec<String> = Vec::new();
    push_terminal_chunk(&mut chunks, 10, "old_data_");
    push_terminal_chunk(&mut chunks, 10, "new_data");
    // The newest content should be preserved
    let result = chunks.concat();
    assert!(result.ends_with("new_data"));
    assert_eq!(js_length(&result), 10);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › edge cases › handles unicode characters correctly (counts characters, not bytes)
#[test]
fn handles_unicode_characters_correctly_counts_characters_not_bytes() {
    let mut chunks: Vec<String> = Vec::new();
    // '日本語' is 3 characters but 9 bytes in UTF-8
    push_terminal_chunk(&mut chunks, 5, "日本語ab");
    // Should count as 5 characters, not 11 bytes
    assert_eq!(chunks, ["日本語ab"]);
    assert_eq!(js_length(&chunks.concat()), 5);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › edge cases › trims unicode characters correctly
#[test]
fn trims_unicode_characters_correctly() {
    let mut chunks: Vec<String> = Vec::new();
    push_terminal_chunk(&mut chunks, 3, "日本語test");
    // 7 characters, capacity 3, need to trim 4 from start
    assert_eq!(chunks, ["est"]);
    assert_eq!(js_length(&chunks.concat()), 3);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › edge cases › handles emoji characters
#[test]
fn handles_emoji_characters() {
    let mut chunks: Vec<String> = Vec::new();
    // Note: some emoji are 2 code units in JS strings
    push_terminal_chunk(&mut chunks, 10, "👋hello");
    // '👋' counts as 2 in JavaScript string length
    assert_eq!(js_length(&chunks.concat()), 7); // 2 + 5
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › edge cases › handles mixed Buffer and string inputs
#[test]
fn handles_mixed_buffer_and_string_inputs() {
    let mut chunks: Vec<String> = Vec::new();
    push_terminal_chunk(&mut chunks, 30, "string_input");
    push_terminal_chunk(&mut chunks, 30, b"_buffer_input");
    assert_eq!(chunks, ["string_input", "_buffer_input"]);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › edge cases › handles newlines and special characters
#[test]
fn handles_newlines_and_special_characters() {
    let mut chunks: Vec<String> = Vec::new();
    push_terminal_chunk(&mut chunks, 20, "line1\nline2\r\n");
    assert_eq!(chunks, ["line1\nline2\r\n"]);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › edge cases › handles ANSI escape sequences
#[test]
fn handles_ansi_escape_sequences() {
    let mut chunks: Vec<String> = Vec::new();
    let ansi_colored = "\x1b[31mred\x1b[0m";
    push_terminal_chunk(&mut chunks, 50, ansi_colored);
    assert_eq!(chunks, [ansi_colored]);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › pre-existing buffer state › works correctly with pre-populated buffer
#[test]
fn works_correctly_with_pre_populated_buffer() {
    let mut chunks: Vec<String> = vec!["existing".into(), "content".into()];
    push_terminal_chunk(&mut chunks, 20, "_new");
    assert_eq!(chunks, ["existing", "content", "_new"]);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › pre-existing buffer state › trims pre-existing content when adding new chunk exceeds capacity
#[test]
fn trims_pre_existing_content_when_adding_new_chunk_exceeds_capacity() {
    let mut chunks: Vec<String> = vec!["aaaa".into(), "bbbb".into()]; // 8 chars
    push_terminal_chunk(&mut chunks, 10, "cccccc"); // would be 14, capacity 10
    // Need to trim 4 chars from start
    // 'aaaa' is removed entirely (4 chars), overrun satisfied
    assert_eq!(chunks, ["bbbb", "cccccc"]);
    assert_eq!(js_length(&chunks.concat()), 10);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › boundary conditions › handles exact capacity boundary
#[test]
fn handles_exact_capacity_boundary() {
    let mut chunks: Vec<String> = vec!["12345".into()];
    push_terminal_chunk(&mut chunks, 10, "67890");
    assert_eq!(chunks, ["12345", "67890"]);
    assert_eq!(js_length(&chunks.concat()), 10);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › boundary conditions › handles one character over capacity
#[test]
fn handles_one_character_over_capacity() {
    let mut chunks: Vec<String> = vec!["12345".into()];
    push_terminal_chunk(&mut chunks, 10, "678901");
    // Total 11, capacity 10, overrun 1
    assert_eq!(js_length(&chunks.concat()), 10);
    assert_eq!(chunks.concat(), "2345678901");
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › boundary conditions › handles very large capacity with small chunks
#[test]
fn handles_very_large_capacity_with_small_chunks() {
    let mut chunks: Vec<String> = Vec::new();
    let capacity = 1000000;
    push_terminal_chunk(&mut chunks, capacity, "small");
    assert_eq!(chunks, ["small"]);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › boundary conditions › handles many small chunks
#[test]
fn handles_many_small_chunks() {
    let mut chunks: Vec<String> = Vec::new();
    let capacity = 20;

    for _ in 0..10 {
        push_terminal_chunk(&mut chunks, capacity, "xx");
    }

    // 10 chunks of 'xx' = 20 chars, exactly at capacity
    assert_eq!(js_length(&chunks.concat()), 20);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › boundary conditions › correctly handles while loop with exact chunk removal
#[test]
fn correctly_handles_while_loop_with_exact_chunk_removal() {
    // Set up scenario where overrun exactly equals first chunk length
    let mut chunks: Vec<String> = vec!["abc".into()]; // 3 chars
    push_terminal_chunk(&mut chunks, 5, "defgh"); // would be 8, capacity 5, overrun exactly 3
    // 'abc' should be removed entirely
    assert_eq!(chunks, ["defgh"]);
    assert_eq!(js_length(&chunks.concat()), 5);
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › realistic terminal output scenarios › simulates git push output
#[test]
fn simulates_git_push_output() {
    let mut chunks: Vec<String> = Vec::new();
    let capacity = 1000;

    push_terminal_chunk(&mut chunks, capacity, "Enumerating objects: 5, done.\n");
    push_terminal_chunk(
        &mut chunks,
        capacity,
        "Counting objects: 100% (5/5), done.\n",
    );
    push_terminal_chunk(
        &mut chunks,
        capacity,
        "Delta compression using up to 8 threads\n",
    );
    push_terminal_chunk(
        &mut chunks,
        capacity,
        "Compressing objects: 100% (3/3), done.\n",
    );
    push_terminal_chunk(
        &mut chunks,
        capacity,
        "Writing objects: 100% (3/3), 328 bytes | 328.00 KiB/s, done.\n",
    );

    let result = chunks.concat();
    assert!(result.contains("Enumerating"));
    assert!(result.contains("Writing objects"));
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › realistic terminal output scenarios › simulates progress output with carriage returns
#[test]
fn simulates_progress_output_with_carriage_returns() {
    let mut chunks: Vec<String> = Vec::new();
    let capacity = 100;

    // Simulate progress updates that overwrite each other
    push_terminal_chunk(&mut chunks, capacity, "Progress: 25%\r");
    push_terminal_chunk(&mut chunks, capacity, "Progress: 50%\r");
    push_terminal_chunk(&mut chunks, capacity, "Progress: 75%\r");
    push_terminal_chunk(&mut chunks, capacity, "Progress: 100%\n");

    let result = chunks.concat();
    assert!(result.contains("Progress: 100%"));
}

// GHD: unit/git/push-terminal-chunk-test.ts › pushTerminalChunk › realistic terminal output scenarios › handles large output that needs significant trimming
#[test]
fn handles_large_output_that_needs_significant_trimming() {
    let mut chunks: Vec<String> = Vec::new();
    let capacity = 100;

    // Add a lot of output
    for i in 0..50 {
        push_terminal_chunk(
            &mut chunks,
            capacity,
            format!("Line {i}: Some output data\n"),
        );
    }

    let result = chunks.concat();
    assert_eq!(js_length(&result), 100);
    // Should contain only the most recent content
    assert!(!result.contains("Line 0:"));
}
