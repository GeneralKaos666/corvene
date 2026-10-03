//! Port of GitHub Desktop's `app/test/unit/git/create-tail-stream-test.ts`.
//!
//! Corvene has no equivalent of `createTailStream(capacity, options)`
//! (`lib/git/create-tail-stream.ts`: a transform stream that keeps the last
//! `capacity` bytes written to it and emits them as one chunk when it
//! ends). In GitHub Desktop 3.6.6 nothing in `app/src` uses it any more;
//! the cases call a stand-in and are ignored until Corvene has one.
//!
//! The node stream becomes [`TailStream`]: `write` takes a chunk and
//! returns the chunks the stream emits for it, `end` returns the chunks it
//! emits when it ends. `{ encoding: 'utf8' }` makes the emitted chunks
//! strings. `Readable.from(chunks).pipe(…).toArray()` is [`write`]: every
//! chunk written in order, then every emitted chunk collected.

/// Stand-in for the stream `createTailStream(capacity, { encoding: 'utf8'
/// })` returns. Replace it with the `corvene_git` type once there is one and
/// remove the `#[ignore]`.
struct TailStream;

/// Stand-in for GitHub Desktop's `createTailStream(capacity, { encoding:
/// 'utf8' })`.
fn create_tail_stream(_capacity: usize) -> TailStream {
    TailStream
}

impl TailStream {
    fn write(&mut self, _chunk: &[u8]) -> Vec<String> {
        unimplemented!("corvene_git has no createTailStream")
    }

    fn end(self) -> Vec<String> {
        unimplemented!("corvene_git has no createTailStream")
    }
}

/// The test's `write(maxLength, ...chunks)`.
fn write(max_length: usize, chunks: &[&str]) -> Vec<String> {
    let mut stream = create_tail_stream(max_length);
    let mut output = Vec::new();
    for chunk in chunks {
        output.extend(stream.write(chunk.as_bytes()));
    }
    output.extend(stream.end());
    output
}

// GHD: unit/git/create-tail-stream-test.ts › createTailStream › only keeps the tail of the input stream
#[test]
#[ignore = "ghd: missing: corvene_git has no createTailStream (lib/git/create-tail-stream.ts, unused in GHD 3.6.6 app/src)"]
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
