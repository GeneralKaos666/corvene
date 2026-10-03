//! Port of GitHub Desktop's
//! `app/test/unit/git/multi-operation-terminal-output-test.ts`.
//!
//! GitHub Desktop's `createMultiOperationTerminalOutputCallback`
//! (`lib/git/multi-operation-terminal-output.ts`, which `merge` uses to show
//! one terminal output for `merge` plus the squash commit) is
//! `corvene_git::create_multi_operation_terminal_output_callback`, and the
//! `onTerminalOutputAvailable` option of GitHub Desktop's `git()`
//! (`lib/git/core.ts`) it plugs into is
//! `corvene_git::GitCommand::run_with_terminal_output`; [`git_with_terminal_output`]
//! is that call.
//!
//! GitHub Desktop's callback types are `corvene_git`'s:
//!
//! - `TerminalOutput` (`string | Buffer | Buffer[]`): `TerminalOutput`,
//! - `TerminalOutputListener` (subscribe a callback, replaying what is
//!   buffered; returns `{ unsubscribe }`): `TerminalOutputListener`,
//! - `TerminalOutputCallback` (receives a listener):
//!   `TerminalOutputCallback`.
//!
//! `__dirname` (the directory `git version` runs in) is this file's
//! directory.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use corvene_git::error::Result;
use corvene_git::{
    GitCommand, GitOutput, TerminalOutput, TerminalOutputCallback, TerminalOutputListener,
    create_multi_operation_terminal_output_callback,
};
use corvene_test_support::init;

/// JavaScript's `.length` of a `TerminalOutput`: bytes of a chunk, elements
/// of an array. GitHub Desktop streams `Buffer`s (bytes) but replays its
/// buffer as strings (UTF-16 code units); `git version` prints ASCII, where
/// both counts are the same.
trait Length {
    fn length(&self) -> usize;
}

impl Length for TerminalOutput {
    fn length(&self) -> usize {
        match self {
            TerminalOutput::Chunk(bytes) => bytes.len(),
            TerminalOutput::Chunks(chunks) => chunks.len(),
        }
    }
}

/// GitHub Desktop's `git(args, path, name, { onTerminalOutputAvailable })`:
/// `GitCommand::run_with_terminal_output`.
fn git_with_terminal_output(
    args: &[&str],
    path: &std::path::Path,
    _name: &str,
    on_terminal_output_available: &TerminalOutputCallback,
) -> Result<GitOutput> {
    GitCommand::new(corvene_test_support::git())
        .args(args)
        .current_dir(path)
        .run_with_terminal_output(on_terminal_output_available)
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
