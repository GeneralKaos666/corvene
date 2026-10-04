//! Port of GitHub Desktop's
//! `app/test/unit/git/create-terminal-stream-test.ts`.
//!
//! GitHub Desktop's `createTerminalStream()` (`lib/create-terminal-stream.ts`:
//! a transform stream that applies `\r` the way a terminal does, so git's
//! progress lines overwrite each other and only the last state of each line
//! remains) is `corvene_git::create_terminal_stream`. In GitHub Desktop
//! 3.6.6 nothing in `app/src` uses it any more.
//!
//! The node stream is `corvene_git::TerminalStream`: `write` takes a chunk
//! and returns the chunks the stream emits for it, `end` returns what it
//! emits when it ends. `Readable.from(ts).toArray()` then `chunks.join('')` is
//! the emitted chunks concatenated and decoded as UTF-8.
//! `createReadStream(path).pipe(ts)` writes the file in 64 KiB chunks (node's
//! default `highWaterMark` for file streams) and ends the stream.

use corvene_git::create_terminal_stream;
use corvene_test_support::get_fixture_path;

/// `chunks.join('')` of the emitted `Buffer`s.
fn join(chunks: Vec<Vec<u8>>) -> String {
    String::from_utf8(chunks.concat()).expect("UTF-8 output")
}

// GHD: unit/git/create-terminal-stream-test.ts › terminal-stream › can handle git clone progress
#[test]
fn can_handle_git_clone_progress() {
    let mut ts = create_terminal_stream();
    let input = std::fs::read(get_fixture_path(["clone-with-progress-output"])).unwrap();
    let mut chunks = Vec::new();
    for chunk in input.chunks(64 * 1024) {
        chunks.extend(ts.write(chunk));
    }
    chunks.extend(ts.end());
    let actual = join(chunks);
    let expected = String::new()
        + "Cloning into 'linux'...\n"
        + "remote: Enumerating objects: 10460179, done.        \n"
        + "remote: Counting objects: 100% (165/165), done.        \n"
        + "remote: Compressing objects: 100% (106/106), done.        \n"
        + "remote: Total 10460179 (delta 94), reused 70 (delta 59), pack-reused 10460014 (from 1)        \n"
        + "Receiving objects: 100% (10460179/10460179), 5.05 GiB | 8.72 MiB/s, done.\n"
        + "Resolving deltas: 100% (8517403/8517403), done.\n"
        + "Updating files: 100% (86676/86676), done.\n";

    assert_eq!(actual, expected);
}

// GHD: unit/git/create-terminal-stream-test.ts › terminal-stream › can handle all kinds of chunk sizes
#[test]
fn can_handle_all_kinds_of_chunk_sizes() {
    let mut ts = create_terminal_stream();
    // `Buffer.alloc(2048).fill('abc…789')`: the pattern repeated
    let pattern = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let buf: Vec<u8> = pattern.iter().copied().cycle().take(2048).collect();
    let mut buffers: Vec<&[u8]> = Vec::new();

    let mut output = Vec::new();

    for i in 1..2048 {
        output.extend(ts.write(&buf[..i]));
        buffers.push(&buf[..i]);
    }

    for i in (1..=2048).rev() {
        output.extend(ts.write(&buf[..i]));
        buffers.push(&buf[..i]);
    }

    output.extend(ts.end());

    let actual = join(output);
    let expected = String::from_utf8(buffers.concat()).unwrap();

    assert_eq!(actual, expected);
}

// GHD: unit/git/create-terminal-stream-test.ts › terminal-stream › can handle empty buffers
#[test]
fn can_handle_empty_buffers() {
    let mut ts = create_terminal_stream();

    let mut output = Vec::new();

    output.extend(ts.write(&[]));
    output.extend(ts.end());

    let actual = join(output);
    let expected = "";

    assert_eq!(actual, expected);
}
