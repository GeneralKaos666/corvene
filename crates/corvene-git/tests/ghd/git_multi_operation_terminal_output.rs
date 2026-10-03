//! Port of GitHub Desktop's
//! `app/test/unit/git/multi-operation-terminal-output-test.ts`.
//!
//! Corvene has no equivalent of `createMultiOperationTerminalOutputCallback`
//! (`lib/git/multi-operation-terminal-output.ts`, which `merge` uses to show
//! one terminal output for `merge` plus the squash commit), nor of the
//! `onTerminalOutputAvailable` option of GitHub Desktop's `git()`
//! (`lib/git/core.ts`) it plugs into: `corvene_git::GitCommand` hands back
//! stdout and stderr when the command ends and has no subscription to its
//! output. The cases use stand-ins for both and are ignored until they
//! exist.
//!
//! GitHub Desktop's callback types become:
//!
//! - `TerminalOutput` (`string | Buffer | Buffer[]`): [`TerminalOutput`],
//! - `TerminalOutputListener` (subscribe a callback, replaying what is
//!   buffered; returns `{ unsubscribe }`): [`TerminalOutputListener`],
//! - `TerminalOutputCallback` (receives a listener):
//!   [`TerminalOutputCallback`].
//!
//! `__dirname` (the directory `git version` runs in) is this file's
//! directory.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use corvene_git::GitOutput;
use corvene_git::error::Result;
use corvene_test_support::init;

/// GitHub Desktop's `TerminalOutput`: a chunk (`string | Buffer`) or
/// several (`Buffer[]`). Only the stand-ins' replacements produce values.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
enum TerminalOutput {
    Chunk(Vec<u8>),
    Chunks(Vec<Vec<u8>>),
}

impl TerminalOutput {
    /// JavaScript's `.length` of the value: bytes of a chunk, elements of an
    /// array. GitHub Desktop streams `Buffer`s (bytes) but replays its
    /// buffer as strings (UTF-16 code units); `git version` prints ASCII,
    /// where both counts are the same.
    fn length(&self) -> usize {
        match self {
            TerminalOutput::Chunk(bytes) => bytes.len(),
            TerminalOutput::Chunks(chunks) => chunks.len(),
        }
    }
}

/// The `{ unsubscribe }` a [`TerminalOutputListener`] returns.
type Unsubscribe = Box<dyn FnOnce()>;

/// GitHub Desktop's `TerminalOutputListener`.
type TerminalOutputListener = Rc<dyn Fn(Box<dyn FnMut(TerminalOutput)>) -> Unsubscribe>;

/// GitHub Desktop's `TerminalOutputCallback`.
type TerminalOutputCallback = Rc<dyn Fn(TerminalOutputListener)>;

/// Stand-in for GitHub Desktop's
/// `createMultiOperationTerminalOutputCallback(onTerminalOutputAvailable,
/// capacity = 256 * 1024)`. Replace it with the `corvene_git` function once
/// there is one and remove the `#[ignore]`s.
fn create_multi_operation_terminal_output_callback(
    _on_terminal_output_available: impl Fn(TerminalOutputListener) + 'static,
    _capacity: Option<usize>,
) -> TerminalOutputCallback {
    unimplemented!("corvene_git has no createMultiOperationTerminalOutputCallback")
}

/// Stand-in for GitHub Desktop's `git(args, path, name, {
/// onTerminalOutputAvailable })`: runs git and hands its combined output to
/// the callback. Replace it with the `corvene_git` equivalent once there is
/// one.
fn git_with_terminal_output(
    _args: &[&str],
    _path: &std::path::Path,
    _name: &str,
    _on_terminal_output_available: &TerminalOutputCallback,
) -> Result<GitOutput> {
    unimplemented!("corvene_git::GitCommand has no onTerminalOutputAvailable")
}

/// GitHub Desktop's `__dirname` in the test file.
fn dirname() -> PathBuf {
    init();
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("ghd")
}

// GHD: unit/git/multi-operation-terminal-output-test.ts › git/multi-operation-terminal-output › streams output from two git operations
#[test]
#[ignore = "ghd: missing: corvene_git has no createMultiOperationTerminalOutputCallback (lib/git/multi-operation-terminal-output.ts) nor git() onTerminalOutputAvailable"]
fn streams_output_from_two_git_operations() {
    let chunks: Rc<RefCell<Vec<TerminalOutput>>> = Rc::default();

    let on_terminal_output_available = {
        let chunks = chunks.clone();
        create_multi_operation_terminal_output_callback(
            move |cb: TerminalOutputListener| {
                let chunks = chunks.clone();
                let _unsubscribe = cb(Box::new(move |chunk| {
                    chunks.borrow_mut().push(chunk);
                }));
            },
            None,
        )
    };

    git_with_terminal_output(&["version"], &dirname(), "", &on_terminal_output_available).unwrap();
    assert_eq!(
        chunks.borrow().len(),
        1,
        "expected output from first git operation"
    );
    git_with_terminal_output(&["version"], &dirname(), "", &on_terminal_output_available).unwrap();
    assert_eq!(
        chunks.borrow().len(),
        2,
        "expected output from second git operation"
    );
}

// GHD: unit/git/multi-operation-terminal-output-test.ts › git/multi-operation-terminal-output › buffers output from two git operations
#[test]
#[ignore = "ghd: missing: corvene_git has no createMultiOperationTerminalOutputCallback (lib/git/multi-operation-terminal-output.ts) nor git() onTerminalOutputAvailable"]
fn buffers_output_from_two_git_operations() {
    let chunks: Rc<RefCell<Vec<TerminalOutput>>> = Rc::default();
    let holder: Rc<RefCell<Option<TerminalOutputListener>>> = Rc::default();

    let on_terminal_output_available = {
        let holder = holder.clone();
        create_multi_operation_terminal_output_callback(
            move |cb| {
                *holder.borrow_mut() = Some(cb);
            },
            None,
        )
    };

    assert!(holder.borrow().is_none(), "expected subscriber to be set");

    git_with_terminal_output(&["version"], &dirname(), "", &on_terminal_output_available).unwrap();
    git_with_terminal_output(&["version"], &dirname(), "", &on_terminal_output_available).unwrap();

    assert!(holder.borrow().is_some(), "expected subscriber to be set");

    let subscribe = holder.borrow().clone().unwrap();
    {
        let chunks = chunks.clone();
        let _unsubscribe = subscribe(Box::new(move |chunk| match chunk {
            TerminalOutput::Chunks(many) => chunks
                .borrow_mut()
                .extend(many.into_iter().map(TerminalOutput::Chunk)),
            chunk => chunks.borrow_mut().push(chunk),
        }));
    }

    assert_eq!(
        chunks.borrow().len(),
        2,
        "expected buffered output from both git operations"
    );
}

// GHD: unit/git/multi-operation-terminal-output-test.ts › git/multi-operation-terminal-output › calls the original callback only once
#[test]
#[ignore = "ghd: missing: corvene_git has no createMultiOperationTerminalOutputCallback (lib/git/multi-operation-terminal-output.ts) nor git() onTerminalOutputAvailable"]
fn calls_the_original_callback_only_once() {
    let callcount = Rc::new(Cell::new(0));

    let on_terminal_output_available = {
        let callcount = callcount.clone();
        create_multi_operation_terminal_output_callback(
            move |_cb| {
                callcount.set(callcount.get() + 1);
            },
            None,
        )
    };

    assert_eq!(callcount.get(), 0, "expected callback to be called once");

    git_with_terminal_output(&["version"], &dirname(), "", &on_terminal_output_available).unwrap();
    assert_eq!(callcount.get(), 1, "expected callback to be called once");
    git_with_terminal_output(&["version"], &dirname(), "", &on_terminal_output_available).unwrap();
    assert_eq!(callcount.get(), 1, "expected callback to be called once");
}

// GHD: unit/git/multi-operation-terminal-output-test.ts › git/multi-operation-terminal-output › streams output untrimmed
#[test]
#[ignore = "ghd: missing: corvene_git has no createMultiOperationTerminalOutputCallback (lib/git/multi-operation-terminal-output.ts) nor git() onTerminalOutputAvailable"]
fn streams_output_untrimmed() {
    let chunks: Rc<RefCell<Vec<TerminalOutput>>> = Rc::default();

    let on_terminal_output_available = {
        let chunks = chunks.clone();
        create_multi_operation_terminal_output_callback(
            move |cb: TerminalOutputListener| {
                let chunks = chunks.clone();
                let _unsubscribe = cb(Box::new(move |chunk| {
                    chunks.borrow_mut().push(chunk);
                }));
            },
            Some(10), // small capacity to trigger trimming
        )
    };

    git_with_terminal_output(&["version"], &dirname(), "", &on_terminal_output_available).unwrap();
    assert_eq!(
        chunks.borrow().len(),
        1,
        "expected output from git operation"
    );
    assert!(
        chunks.borrow()[0].length() > 10,
        "expected untrimmed output from git operation"
    );
}

// GHD: unit/git/multi-operation-terminal-output-test.ts › git/multi-operation-terminal-output › trims buffered output
#[test]
#[ignore = "ghd: missing: corvene_git has no createMultiOperationTerminalOutputCallback (lib/git/multi-operation-terminal-output.ts) nor git() onTerminalOutputAvailable"]
fn trims_buffered_output() {
    let chunks: Rc<RefCell<Vec<TerminalOutput>>> = Rc::default();
    let holder: Rc<RefCell<Option<TerminalOutputListener>>> = Rc::default();

    let on_terminal_output_available = {
        let holder = holder.clone();
        create_multi_operation_terminal_output_callback(
            move |cb| {
                *holder.borrow_mut() = Some(cb);
            },
            Some(10), // small capacity to trigger trimming
        )
    };

    assert!(holder.borrow().is_none(), "expected subscriber to be set");

    git_with_terminal_output(&["version"], &dirname(), "", &on_terminal_output_available).unwrap();
    git_with_terminal_output(&["version"], &dirname(), "", &on_terminal_output_available).unwrap();

    assert!(holder.borrow().is_some(), "expected subscriber to be set");

    let subscribe = holder.borrow().clone().unwrap();
    {
        let chunks = chunks.clone();
        let _unsubscribe = subscribe(Box::new(move |chunk| match chunk {
            TerminalOutput::Chunks(many) => chunks
                .borrow_mut()
                .extend(many.into_iter().map(TerminalOutput::Chunk)),
            chunk => chunks.borrow_mut().push(chunk),
        }));
    }

    assert_eq!(chunks.borrow().len(), 1, "expected buffered output");
    assert_eq!(
        chunks.borrow()[0].length(),
        10,
        "expected buffered output to be trimmed to capacity"
    );
}

// GHD: unit/git/multi-operation-terminal-output-test.ts › git/multi-operation-terminal-output › handles multiple subscribers
#[test]
#[ignore = "ghd: missing: corvene_git has no createMultiOperationTerminalOutputCallback (lib/git/multi-operation-terminal-output.ts) nor git() onTerminalOutputAvailable"]
fn handles_multiple_subscribers() {
    let holder: Rc<RefCell<Option<TerminalOutputListener>>> = Rc::default();

    let on_terminal_output_available = {
        let holder = holder.clone();
        create_multi_operation_terminal_output_callback(
            move |cb| {
                *holder.borrow_mut() = Some(cb);
            },
            Some(10), // small capacity to trigger trimming
        )
    };

    assert!(holder.borrow().is_none(), "expected subscriber to be set");

    git_with_terminal_output(&["version"], &dirname(), "", &on_terminal_output_available).unwrap();
    git_with_terminal_output(&["version"], &dirname(), "", &on_terminal_output_available).unwrap();

    assert!(holder.borrow().is_some(), "expected subscriber to be set");

    let subscriber_one_last_chunk: Rc<RefCell<Option<TerminalOutput>>> = Rc::default();
    let subscriber_two_last_chunk: Rc<RefCell<Option<TerminalOutput>>> = Rc::default();

    let subscribe = holder.borrow().clone().unwrap();
    {
        let last = subscriber_one_last_chunk.clone();
        let _unsubscribe = subscribe(Box::new(move |chunk| *last.borrow_mut() = Some(chunk)));
    }
    {
        let last = subscriber_two_last_chunk.clone();
        let _unsubscribe = subscribe(Box::new(move |chunk| *last.borrow_mut() = Some(chunk)));
    }

    assert!(
        subscriber_one_last_chunk.borrow().is_some(),
        "expected subscriber one to receive chunk"
    );
    assert!(
        subscriber_two_last_chunk.borrow().is_some(),
        "expected subscriber two to receive chunk"
    );
    assert_eq!(
        *subscriber_one_last_chunk.borrow(),
        *subscriber_two_last_chunk.borrow()
    );
}
