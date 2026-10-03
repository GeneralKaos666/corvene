//! Port of GitHub Desktop's `app/test/unit/git/create-tail-stream-test.ts`.
//!
//! GitHub Desktop's `createTailStream(capacity, options)`
//! (`lib/git/create-tail-stream.ts`: a transform stream that keeps the last
//! `capacity` bytes written to it and emits them as one chunk when it ends)
//! is `corvene_git::create_tail_stream`. In GitHub Desktop 3.6.6 nothing in
//! `app/src` uses it any more.
//!
//! The node stream is `corvene_git::TailStream`: `write` takes a chunk and
//! returns the chunks the stream emits for it, `end` returns the chunks it
//! emits when it ends. `{ encoding: 'utf8' }` makes the emitted chunks
//! strings: here each emitted chunk decoded as UTF-8.
//! `Readable.from(chunks).pipe(…).toArray()` is [`write`]: every chunk
//! written in order, then every emitted chunk collected.

use corvene_git::create_tail_stream;

/// The test's `write(maxLength, ...chunks)`.
fn write(max_length: usize, chunks: &[&str]) -> Vec<String> {
    let mut stream = create_tail_stream(max_length).expect("a capacity above 0");
    let mut output = Vec::new();
    for chunk in chunks {
        output.extend(stream.write(chunk.as_bytes()));
    }
    output.extend(stream.end());
    output
        .into_iter()
        .map(|chunk| String::from_utf8(chunk).expect("UTF-8 output"))
        .collect()
}

// GHD: unit/git/create-tail-stream-test.ts › createTailStream › only keeps the tail of the input stream
#[test]
fn only_keeps_the_tail_of_the_input_stream() {
    assert_eq!(write(3, &["hello"]), ["llo"]);
    assert_eq!(write(5, &["hello"]), ["hello"]);
    assert_eq!(write(10, &["hello", "world"]), ["helloworld"]);
    assert_eq!(write(8, &["hello", "world"]), ["lloworld"]);
    assert_eq!(
        write(10, &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"]),
        ["0123456789"]
    );
    assert_eq!(
        write(8, &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"]),
        ["23456789"]
    );

    let spread: Vec<String> = "helloworld".chars().map(String::from).collect();
    let spread: Vec<&str> = spread.iter().map(String::as_str).collect();
    assert_eq!(write(8, &spread), ["lloworld"]);
}
