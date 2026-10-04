//! A git command's terminal output: GitHub Desktop's rolling buffer of the
//! combined stdout and stderr (`lib/git/push-terminal-chunk.ts`), the
//! subscription to a running command's output it hands to
//! `onTerminalOutputAvailable` (`lib/git/core.ts`) and the aggregator that
//! presents several commands as one output
//! (`lib/git/multi-operation-terminal-output.ts`), plus two stream helpers
//! GitHub Desktop keeps in its library: `createTailStream`
//! (`lib/git/create-tail-stream.ts`) and `createTerminalStream`
//! (`lib/create-terminal-stream.ts`).
//!
//! [`crate::GitCommand`] keeps the last [`TERMINAL_CAPACITY`] of a command's
//! output with [`push_terminal_chunk`] for the error a failed command
//! reports, as GitHub Desktop's `git()` does.
//! [`crate::GitCommand::run_with_terminal_output`] is `git()` with an
//! `onTerminalOutputAvailable` option. In GitHub Desktop only the commit
//! progress view (`ui/commit-progress`, fed by the hooks interception of
//! Settings › Git › Hooks) subscribes to it; Corvene has no such view, so
//! nothing in the application subscribes yet. Neither stream helper has a
//! caller in GitHub Desktop 3.6.6's `app/src`; they are ported with their
//! tests.
//!
//! Lengths are JavaScript string lengths (UTF-16 code units), as in GitHub
//! Desktop: an emoji outside the Basic Multilingual Plane counts as 2.
//! Callbacks are `Rc` closures run on the thread that runs the command, as
//! GitHub Desktop runs them on its event loop.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

/// The capacity of the terminal output buffer of one git command (GitHub
/// Desktop's `terminalCapacity`), and the default of
/// [`create_multi_operation_terminal_output_callback`].
pub const TERMINAL_CAPACITY: usize = 256 * 1024;

/// JavaScript's `string.length` of `s`: its UTF-16 code units.
pub(crate) fn js_length(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// JavaScript's `s.substring(units)`: `s` without its first `units` UTF-16
/// code units. Where that cuts a surrogate pair in two, JavaScript keeps a
/// lone low surrogate, which a Rust string cannot hold: it becomes U+FFFD,
/// one code unit as well, so the length stays JavaScript's.
fn js_substring(s: &str, units: usize) -> String {
    let mut skipped = 0;
    for (at, c) in s.char_indices() {
        if skipped == units {
            return s[at..].to_string();
        }
        let len = c.len_utf16();
        if skipped + len > units {
            return format!("\u{FFFD}{}", &s[at + c.len_utf8()..]);
        }
        skipped += len;
    }
    String::new()
}

/// Drops or trims chunks from the front of `chunks`, whose total length is
/// `length`, until it is at most `capacity`; returns the new total.
fn trim_front(chunks: &mut Vec<String>, mut length: usize, capacity: usize) -> usize {
    while length > capacity {
        let Some(first) = chunks.first() else {
            return 0;
        };
        let first_length = js_length(first);
        let overrun = length - capacity;
        if overrun >= first_length {
            chunks.remove(0);
            length -= first_length;
        } else {
            chunks[0] = js_substring(first, overrun);
            length -= overrun;
        }
    }
    length
}

/// GitHub Desktop's `pushTerminalChunk(chunks, capacity, chunk)`: appends
/// `chunk` (bytes are decoded as UTF-8, invalid sequences as U+FFFD) and
/// then drops chunks from the front, trimming the last one it reaches, until
/// the buffer holds at most `capacity` UTF-16 code units.
pub fn push_terminal_chunk(chunks: &mut Vec<String>, capacity: usize, chunk: impl AsRef<[u8]>) {
    chunks.push(String::from_utf8_lossy(chunk.as_ref()).into_owned());
    let length = chunks.iter().map(|c| js_length(c)).sum();
    trim_front(chunks, length, capacity);
}

/// [`push_terminal_chunk`] with the running total kept, for the buffers
/// that grow with every chunk a command writes.
#[derive(Debug, Default)]
pub(crate) struct TerminalBuffer {
    chunks: Vec<String>,
    length: usize,
    capacity: usize,
}

impl TerminalBuffer {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            chunks: Vec::new(),
            length: 0,
            capacity,
        }
    }

    pub(crate) fn push(&mut self, chunk: &[u8]) {
        let chunk = String::from_utf8_lossy(chunk).into_owned();
        self.length += js_length(&chunk);
        self.chunks.push(chunk);
        self.length = trim_front(&mut self.chunks, self.length, self.capacity);
    }

    pub(crate) fn chunks(&self) -> &[String] {
        &self.chunks
    }

    /// GitHub Desktop's `terminalChunks.join('')`.
    pub(crate) fn joined(&self) -> String {
        self.chunks.concat()
    }
}

/// GitHub Desktop's `TerminalOutput` (`string | Buffer | Buffer[]`): one
/// chunk of output, or several (`Buffer[]`, what the hooks interception
/// hands over; a git command streams single chunks).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalOutput {
    Chunk(Vec<u8>),
    Chunks(Vec<Vec<u8>>),
}

/// Stops a subscription (GitHub Desktop's `{ unsubscribe }`).
pub type Unsubscribe = Box<dyn FnOnce()>;

/// What receives a command's output.
pub type TerminalOutputSubscriber = Box<dyn FnMut(TerminalOutput)>;

/// GitHub Desktop's `TerminalOutputListener`: subscribes a callback, which
/// first receives the output buffered so far and then every chunk as it is
/// written.
pub type TerminalOutputListener = Rc<dyn Fn(TerminalOutputSubscriber) -> Unsubscribe>;

/// GitHub Desktop's `TerminalOutputCallback`: handed the listener of a
/// command once the command has started.
pub type TerminalOutputCallback = Rc<dyn Fn(TerminalOutputListener)>;

/// The buffered output and the subscribers of one listener.
struct Subscribers {
    buffer: TerminalBuffer,
    subscribers: Vec<(u64, TerminalOutputSubscriber)>,
    next_id: u64,
}

/// A listener over a buffer that [`LiveOutput::push`] fills: GitHub
/// Desktop's replay of `terminalChunks` followed by the process's `data`
/// events.
pub(crate) struct LiveOutput(Rc<RefCell<Subscribers>>);

impl LiveOutput {
    pub(crate) fn new(capacity: usize) -> Self {
        Self(Rc::new(RefCell::new(Subscribers {
            buffer: TerminalBuffer::new(capacity),
            subscribers: Vec::new(),
            next_id: 0,
        })))
    }

    /// The listener handed to `onTerminalOutputAvailable`.
    pub(crate) fn listener(&self) -> TerminalOutputListener {
        let state = Rc::downgrade(&self.0);
        Rc::new(move |mut subscriber: TerminalOutputSubscriber| {
            let Some(strong) = state.upgrade() else {
                return Box::new(|| {}) as Unsubscribe;
            };
            // replay without holding the state: a subscriber may subscribe again
            let replay: Vec<String> = strong.borrow().buffer.chunks().to_vec();
            for chunk in replay {
                subscriber(TerminalOutput::Chunk(chunk.into_bytes()));
            }
            let id = {
                let mut s = strong.borrow_mut();
                let id = s.next_id;
                s.next_id += 1;
                s.subscribers.push((id, subscriber));
                id
            };
            unsubscribe(Rc::downgrade(&strong), id)
        })
    }

    /// One chunk the command wrote: to every subscriber, then into the
    /// buffer.
    pub(crate) fn push(&self, chunk: &[u8]) {
        dispatch(&self.0, &TerminalOutput::Chunk(chunk.to_vec()));
        self.0.borrow_mut().buffer.push(chunk);
    }

    /// The buffered output, joined.
    pub(crate) fn joined(&self) -> String {
        self.0.borrow().buffer.joined()
    }
}

fn unsubscribe(state: Weak<RefCell<Subscribers>>, id: u64) -> Unsubscribe {
    Box::new(move || {
        if let Some(state) = state.upgrade() {
            state.borrow_mut().subscribers.retain(|(i, _)| *i != id);
        }
    })
}

/// Calls every subscriber with `output`. The subscribers are taken out of
/// the state meanwhile, so one may subscribe or unsubscribe from inside.
fn dispatch(state: &Rc<RefCell<Subscribers>>, output: &TerminalOutput) {
    let mut subscribers = std::mem::take(&mut state.borrow_mut().subscribers);
    for (_, subscriber) in &mut subscribers {
        subscriber(output.clone());
    }
    let mut s = state.borrow_mut();
    // subscribed meanwhile: after the ones that were there
    let added = std::mem::take(&mut s.subscribers);
    subscribers.extend(added);
    s.subscribers = subscribers;
}

/// GitHub Desktop's `createMultiOperationTerminalOutputCallback(
/// onTerminalOutputAvailable, capacity = 256 * 1024)`: a callback to hand
/// to several git commands run one after the other, presenting their output
/// as one. `on_terminal_output_available` is called once, when the first
/// command writes something; its subscribers receive what was buffered
/// (the last `capacity`, [`TERMINAL_CAPACITY`] when `None`) and then every
/// chunk untrimmed.
///
/// A `Chunks` value (several chunks at once) is buffered chunk by chunk.
/// GitHub Desktop pushes its own buffer again there instead
/// (`chunks.forEach(push)`), a slip no git command reaches: they write
/// single chunks.
pub fn create_multi_operation_terminal_output_callback(
    on_terminal_output_available: impl Fn(TerminalOutputListener) + 'static,
    capacity: Option<usize>,
) -> TerminalOutputCallback {
    struct Aggregate {
        output_started: bool,
        live: LiveOutput,
        upstream: Box<dyn Fn(TerminalOutputListener)>,
    }
    let aggregate = Rc::new(RefCell::new(Aggregate {
        output_started: false,
        live: LiveOutput::new(capacity.unwrap_or(TERMINAL_CAPACITY)),
        upstream: Box::new(on_terminal_output_available),
    }));
    let push = Rc::new(move |chunk: &[u8]| {
        let started = std::mem::replace(&mut aggregate.borrow_mut().output_started, true);
        if !started {
            let listener = aggregate.borrow().live.listener();
            // not borrowed while upstream runs: it may subscribe at once
            let upstream =
                std::mem::replace(&mut aggregate.borrow_mut().upstream, Box::new(|_| {}));
            upstream(listener);
            aggregate.borrow_mut().upstream = upstream;
        }
        let live = Rc::clone(&aggregate.borrow().live.0);
        live.borrow_mut().buffer.push(chunk);
        dispatch(&live, &TerminalOutput::Chunk(chunk.to_vec()));
    });
    Rc::new(move |subscribe: TerminalOutputListener| {
        let push = Rc::clone(&push);
        // never unsubscribed: whoever subscribes upstream later must still
        // see the output of every command
        let _unsubscribe = subscribe(Box::new(move |output| match output {
            TerminalOutput::Chunk(chunk) => push(&chunk),
            TerminalOutput::Chunks(chunks) => chunks.iter().for_each(|chunk| push(chunk)),
        }));
    })
}

/// GitHub Desktop's `createTailStream(capacity)`: keeps the last `capacity`
/// bytes written to it and emits them as one chunk when it ends.
#[derive(Debug)]
pub struct TailStream {
    chunks: Vec<Vec<u8>>,
    length: usize,
    capacity: usize,
}

/// GitHub Desktop's `createTailStream(capacity)`; `None` for a capacity of
/// 0, where GitHub Desktop's assertion throws.
pub fn create_tail_stream(capacity: usize) -> Option<TailStream> {
    (capacity > 0).then(|| TailStream {
        chunks: Vec::new(),
        length: 0,
        capacity,
    })
}

impl TailStream {
    /// Writes `chunk`; the stream emits nothing until it ends.
    pub fn write(&mut self, chunk: &[u8]) -> Vec<Vec<u8>> {
        self.chunks.push(chunk.to_vec());
        self.length += chunk.len();
        while self.length > self.capacity {
            let Some(first_length) = self.chunks.first().map(Vec::len) else {
                break;
            };
            let overrun = self.length - self.capacity;
            if overrun >= first_length {
                self.chunks.remove(0);
                self.length -= first_length;
            } else {
                self.chunks[0].drain(..overrun);
                self.length -= overrun;
            }
        }
        Vec::new()
    }

    /// Ends the stream: the kept tail as one chunk (`Buffer.concat`).
    pub fn end(self) -> Vec<Vec<u8>> {
        vec![self.chunks.concat()]
    }
}

/// The width of [`TerminalStream`]'s line.
const TERMINAL_LINE: usize = 1024;

/// GitHub Desktop's `createTerminalStream()`: applies `\r` the way a
/// terminal does, so progress that git rewrites in place leaves only its
/// last state. The stream keeps one 1 KiB line: `\r` moves the cursor to its
/// start (what follows overwrites it), `\n` emits the line (with the `\n`
/// and any `\r` written into it), and a line that fills up is emitted and
/// continued on a fresh one, as a terminal wraps. Byte for byte GitHub
/// Desktop's transform, including its copying of the whole rest of a chunk
/// into the line where only the part up to the next `\r` or `\n` counts.
#[derive(Debug)]
pub struct TerminalStream {
    /// The line (`buf`).
    line: Vec<u8>,
    /// How far the line was written (`l`).
    written: usize,
    /// The cursor (`p`).
    cursor: usize,
}

/// GitHub Desktop's `createTerminalStream()`.
pub fn create_terminal_stream() -> TerminalStream {
    TerminalStream {
        line: vec![0; TERMINAL_LINE],
        written: 0,
        cursor: 0,
    }
}

impl TerminalStream {
    fn reset(&mut self) {
        self.line = vec![0; TERMINAL_LINE];
        self.written = 0;
        self.cursor = 0;
    }

    /// Writes `chunk`; returns the chunks the stream emits for it.
    pub fn write(&mut self, chunk: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let mut push = |bytes: &[u8]| {
            if !bytes.is_empty() {
                out.push(bytes.to_vec());
            }
        };
        let find = |from: usize, byte: u8| chunk[from..].iter().position(|b| *b == byte);
        let last = chunk.len().saturating_sub(1);
        let mut i = 0;
        while i < chunk.len() {
            let cr = find(i, b'\r').map(|at| at + i);
            let lf = match cr {
                // no carriage return: jump to the last line feed
                None => chunk[i..]
                    .iter()
                    .rposition(|b| *b == b'\n')
                    .map(|at| at + i),
                Some(_) => find(i, b'\n').map(|at| at + i),
            };
            let next = cr.unwrap_or(last).min(lf.unwrap_or(last));
            let end = next + 1;
            let mut start = i;
            while end > start {
                let slice_length = end - start;
                if self.cursor + slice_length > self.line.len() {
                    // the line wraps
                    if self.cursor > 0 {
                        push(&self.line[..self.cursor]);
                    }
                    let remaining = self.line.len() - self.cursor;
                    push(&chunk[start..start + remaining]);
                    start += remaining;
                    self.reset();
                } else {
                    // `chunk.copy(buf, p, start)`: the rest of the chunk, as
                    // far as the line has room
                    let copied = (chunk.len() - start).min(self.line.len() - self.cursor);
                    self.line[self.cursor..self.cursor + copied]
                        .copy_from_slice(&chunk[start..start + copied]);
                    self.cursor += slice_length;
                    self.written = self.written.max(self.cursor);
                    break;
                }
            }
            if chunk[next] == b'\n' && self.written > 0 {
                push(&self.line[..self.written]);
                self.reset();
            } else if chunk[next] == b'\r' {
                self.cursor = 0;
            }
            i = next + 1;
        }
        out
    }

    /// Ends the stream: the line written so far, if any.
    pub fn end(self) -> Vec<Vec<u8>> {
        if self.written > 0 {
            vec![self.line[..self.written].to_vec()]
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substring_counts_utf16_units() {
        assert_eq!(js_substring("日本語test", 4), "est");
        assert_eq!(js_substring("👋hello", 2), "hello");
        // half an emoji: a lone surrogate in JavaScript
        assert_eq!(js_substring("👋hello", 1), "\u{FFFD}hello");
        assert_eq!(js_length(&js_substring("👋hello", 1)), 6);
        assert_eq!(js_substring("abc", 3), "");
    }

    #[test]
    fn terminal_buffer_keeps_the_running_length() {
        let mut buffer = TerminalBuffer::new(5);
        buffer.push(b"abc");
        buffer.push(b"defg");
        assert_eq!(buffer.joined(), "cdefg");
        assert_eq!(buffer.length, 5);
    }

    #[test]
    fn terminal_stream_overwrites_on_carriage_return() {
        let mut ts = create_terminal_stream();
        let mut out = ts.write(b"1%\r2%\r100%, done.\nnext");
        out.extend(ts.end());
        assert_eq!(out.concat(), b"100%, done.\nnext");
    }

    #[test]
    fn live_output_replays_then_streams() {
        let live = LiveOutput::new(4);
        live.push(b"ab");
        live.push(b"cdef");
        let seen: Rc<RefCell<Vec<TerminalOutput>>> = Rc::default();
        let listener = live.listener();
        let unsubscribe = {
            let seen = Rc::clone(&seen);
            listener(Box::new(move |chunk| seen.borrow_mut().push(chunk)))
        };
        live.push(b"g");
        unsubscribe();
        live.push(b"h");
        assert_eq!(
            *seen.borrow(),
            vec![
                TerminalOutput::Chunk(b"cdef".to_vec()),
                TerminalOutput::Chunk(b"g".to_vec()),
            ]
        );
        assert_eq!(live.joined(), "efgh");
    }
}
