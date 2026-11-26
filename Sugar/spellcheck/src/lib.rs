//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! The prompt editor's spellchecker: one Hunspell dictionary held by spellbook, driven from a
//! Web Worker through a small C ABI.
//!
//! Strings cross the boundary as UTF-8 bytes the worker writes into memory it got from
//! [`spell_alloc`]. Answers longer than a number are written to one output buffer that
//! [`spell_output`] points at; it stays valid until the next call that writes it.

use std::cell::RefCell;

use foldhash::fast::FixedState;
use spellbook::Dictionary;

/// What one worker's module holds: the loaded dictionary and the last written answer.
#[derive(Default)]
struct Speller {
    dictionary: Option<Dictionary<FixedState>>,
    output: Vec<u8>,
}

thread_local! {
    static SPELLER: RefCell<Speller> = RefCell::new(Speller::default());
}

/// Read `len` bytes at `ptr` as text; `None` when they are not UTF-8.
///
/// # Safety
/// `ptr` must point at `len` initialized bytes the caller owns for the duration of the call.
unsafe fn text<'a>(ptr: *const u8, len: usize) -> Option<&'a str> {
    if len == 0 {
        return Some("");
    }
    std::str::from_utf8(std::slice::from_raw_parts(ptr, len)).ok()
}

/// Whether the dictionary accepts `word` as written or, since the editor checks case-folded
/// words, with its first letter capitalized, so a proper noun such as "victorian" is accepted.
fn accepts(dictionary: &Dictionary<FixedState>, word: &str) -> bool {
    if dictionary.check(word) {
        return true;
    }
    let mut letters = word.chars();
    match letters.next() {
        Some(first) if first.is_lowercase() => {
            let capitalized: String = first.to_uppercase().chain(letters).collect();
            dictionary.check(&capitalized)
        }
        _ => false,
    }
}

/// Reserve `len` bytes for the worker to write a string into.
#[no_mangle]
pub extern "C" fn spell_alloc(len: usize) -> *mut u8 {
    let mut buffer = Vec::<u8>::with_capacity(len.max(1));
    let ptr = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    ptr
}

/// Release bytes reserved by [`spell_alloc`] with the same length.
///
/// # Safety
/// `ptr` and `len` must come from one [`spell_alloc`] call, released once.
#[no_mangle]
pub unsafe extern "C" fn spell_free(ptr: *mut u8, len: usize) {
    drop(Vec::from_raw_parts(ptr, 0, len.max(1)));
}

/// Load a dictionary from its affix and word files; 0 when loaded, -1 when either is invalid.
///
/// # Safety
/// Both ranges must be readable for the call, as for [`text`].
#[no_mangle]
pub unsafe extern "C" fn spell_load(
    aff_ptr: *const u8,
    aff_len: usize,
    dic_ptr: *const u8,
    dic_len: usize,
) -> i32 {
    let (Some(aff), Some(dic)) = (text(aff_ptr, aff_len), text(dic_ptr, dic_len)) else {
        return -1;
    };
    match Dictionary::new_with_hasher(aff, dic, FixedState::default()) {
        Ok(dictionary) => {
            SPELLER.with(|speller| speller.borrow_mut().dictionary = Some(dictionary));
            0
        }
        Err(_) => -1,
    }
}

/// Check newline-separated words; writes one byte per word (`1` accepted, `0` misspelled) to the
/// output and returns how many, or -1 without a dictionary or with invalid text.
///
/// # Safety
/// The range must be readable for the call, as for [`text`].
#[no_mangle]
pub unsafe extern "C" fn spell_check_words(ptr: *const u8, len: usize) -> i32 {
    let Some(words) = text(ptr, len) else {
        return -1;
    };
    SPELLER.with(|speller| {
        let speller = &mut *speller.borrow_mut();
        let Some(dictionary) = speller.dictionary.as_ref() else {
            return -1;
        };
        speller.output.clear();
        if !words.is_empty() {
            for word in words.split('\n') {
                speller
                    .output
                    .push(if accepts(dictionary, word) { b'1' } else { b'0' });
            }
        }
        speller.output.len() as i32
    })
}

/// Write up to `limit` corrections for one word to the output, newline-separated, and return the
/// byte length, or -1 without a dictionary or with invalid text.
///
/// # Safety
/// The range must be readable for the call, as for [`text`].
#[no_mangle]
pub unsafe extern "C" fn spell_suggest(ptr: *const u8, len: usize, limit: usize) -> i32 {
    let Some(word) = text(ptr, len) else {
        return -1;
    };
    SPELLER.with(|speller| {
        let speller = &mut *speller.borrow_mut();
        let Some(dictionary) = speller.dictionary.as_ref() else {
            return -1;
        };
        let mut suggestions = Vec::new();
        dictionary.suggest(word, &mut suggestions);
        suggestions.truncate(limit);
        speller.output.clear();
        speller.output.extend_from_slice(suggestions.join("\n").as_bytes());
        speller.output.len() as i32
    })
}

/// Accept one word as correctly spelled from now on; 0 when added, -1 otherwise.
///
/// # Safety
/// The range must be readable for the call, as for [`text`].
#[no_mangle]
pub unsafe extern "C" fn spell_add(ptr: *const u8, len: usize) -> i32 {
    let Some(word) = text(ptr, len) else {
        return -1;
    };
    SPELLER.with(|speller| match speller.borrow_mut().dictionary.as_mut() {
        Some(dictionary) => match dictionary.add(word) {
            Ok(()) => 0,
            Err(_) => -1,
        },
        None => -1,
    })
}

/// Where the last written answer starts.
#[no_mangle]
pub extern "C" fn spell_output() -> *const u8 {
    SPELLER.with(|speller| speller.borrow().output.as_ptr())
}
