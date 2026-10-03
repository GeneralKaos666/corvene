//! The Windows spell checker (`ISpellChecker`), which Chromium checks with
//! on Windows. COM objects belong to the thread that made them, so one
//! thread owns the checker and answers requests.

use std::ops::Range;
use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};

use windows::Win32::Foundation::S_OK;
use windows::Win32::Globalization::{
    GetUserDefaultLocaleName, ISpellChecker, ISpellCheckerFactory, SpellCheckerFactory,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
};
use windows::core::{HSTRING, PWSTR};

enum Request {
    Check(String, Sender<Vec<Range<usize>>>),
    Suggest(String, Sender<Vec<String>>),
    Learn(String),
}

/// The checker thread, or `None` when Windows has no checker for the
/// user's language or for US English.
fn worker() -> Option<&'static Sender<Request>> {
    static WORKER: OnceLock<Option<Sender<Request>>> = OnceLock::new();
    WORKER
        .get_or_init(|| {
            let (tx, rx) = channel::<Request>();
            let (ready_tx, ready_rx) = channel::<bool>();
            std::thread::Builder::new()
                .name("spellcheck".into())
                .spawn(move || {
                    let checker = create_checker();
                    let _ = ready_tx.send(checker.is_some());
                    let Some(checker) = checker else {
                        return;
                    };
                    for request in rx {
                        match request {
                            Request::Check(text, reply) => {
                                let _ = reply.send(check(&checker, &text));
                            }
                            Request::Suggest(word, reply) => {
                                let _ = reply.send(suggest(&checker, &word));
                            }
                            // SAFETY: a call on the checker this thread made
                            Request::Learn(word) => unsafe {
                                let _ = checker.Add(&HSTRING::from(word));
                            },
                        }
                    }
                })
                .ok()?;
            ready_rx.recv().ok()?.then_some(tx)
        })
        .as_ref()
}

fn create_checker() -> Option<ISpellChecker> {
    // SAFETY: COM calls on this thread, which initializes COM first and
    // lives as long as the process
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let factory: ISpellCheckerFactory =
            CoCreateInstance(&SpellCheckerFactory, None, CLSCTX_INPROC_SERVER).ok()?;
        let mut name = [0u16; 85];
        let len = GetUserDefaultLocaleName(&mut name);
        let user = usize::try_from(len)
            .ok()
            .and_then(|len| len.checked_sub(1))
            .and_then(|len| String::from_utf16(&name[..len]).ok());
        let checker = user
            .into_iter()
            .chain(["en-US".to_string()])
            .find_map(|language| {
                let language = HSTRING::from(language);
                factory
                    .IsSupported(&language)
                    .is_ok_and(|supported| supported.as_bool())
                    .then(|| factory.CreateSpellChecker(&language).ok())
                    .flatten()
            });
        if checker.is_none() {
            tracing::info!("Windows has no spelling dictionary: spellcheck is off");
        }
        checker
    }
}

/// Byte offset of UTF-16 offset `n` in `text`.
fn utf16_to_byte(text: &str, n: usize) -> usize {
    let mut units = 0;
    for (byte, c) in text.char_indices() {
        if units >= n {
            return byte;
        }
        units += c.len_utf16();
    }
    text.len()
}

fn check(checker: &ISpellChecker, text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    // SAFETY: calls on the checker this thread made; the enumerator hands
    // out one error per call until it has none
    unsafe {
        let Ok(errors) = checker.Check(&HSTRING::from(text)) else {
            return out;
        };
        loop {
            let mut error = None;
            if errors.Next(&raw mut error) != S_OK {
                break;
            }
            let Some(error) = error else {
                break;
            };
            let (Ok(start), Ok(len)) = (error.StartIndex(), error.Length()) else {
                break;
            };
            let (start, len) = (start as usize, len as usize);
            out.push(utf16_to_byte(text, start)..utf16_to_byte(text, start + len));
        }
    }
    out
}

/// Chromium shows up to five suggestions.
fn suggest(checker: &ISpellChecker, word: &str) -> Vec<String> {
    let mut out = Vec::new();
    // SAFETY: calls on the checker this thread made; each string the
    // enumerator returns is ours to free
    unsafe {
        let Ok(suggestions) = checker.Suggest(&HSTRING::from(word)) else {
            return out;
        };
        while out.len() < 5 {
            let mut item = [PWSTR::null()];
            let mut fetched = 0u32;
            if suggestions.Next(&mut item, Some(&raw mut fetched)).is_err()
                || fetched == 0
                || item[0].is_null()
            {
                break;
            }
            if let Ok(suggestion) = item[0].to_string() {
                out.push(suggestion);
            }
            CoTaskMemFree(Some(item[0].as_ptr().cast()));
        }
    }
    out
}

pub fn misspelled_ranges(text: &str) -> Vec<Range<usize>> {
    let Some(worker) = worker() else {
        return Vec::new();
    };
    let (tx, rx) = channel();
    if worker.send(Request::Check(text.to_string(), tx)).is_err() {
        return Vec::new();
    }
    rx.recv().unwrap_or_default()
}

pub fn guesses(word: &str) -> Vec<String> {
    let Some(worker) = worker() else {
        return Vec::new();
    };
    let (tx, rx) = channel();
    if worker.send(Request::Suggest(word.to_string(), tx)).is_err() {
        return Vec::new();
    }
    rx.recv().unwrap_or_default()
}

/// "Add to dictionary": Windows keeps the word in the user's dictionary.
pub fn learn_word(word: &str) {
    if let Some(worker) = worker() {
        let _ = worker.send(Request::Learn(word.to_string()));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn utf16_offsets_become_byte_offsets() {
        let text = "né 😀 teh";
        assert_eq!(super::utf16_to_byte(text, 0), 0);
        assert_eq!(super::utf16_to_byte(text, 3), 4);
        // the emoji is two UTF-16 units and four bytes
        assert_eq!(super::utf16_to_byte(text, 6), 9);
        assert_eq!(super::utf16_to_byte(text, 9), text.len());
    }
}
