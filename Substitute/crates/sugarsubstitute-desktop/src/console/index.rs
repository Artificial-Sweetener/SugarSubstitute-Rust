// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep native UTF-16 offsets tied to bounded immutable record identities.
use super::ConsoleText;
mod retained;
use crate::DesktopError;
use sugarsubstitute_contracts::{
    execution::MemoryClaim, lifecycle::ResourceKey, presentation::NATIVE_CONSOLE_STORAGE_BYTES,
    transcript::*,
};
/// Preserve a record-local UTF-16 position across trimming and current-line replacement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Anchor {
    pub(crate) record: RecordId,
    pub(crate) units: u32,
}
#[derive(Clone, Copy)]
struct Row {
    id: RecordId,
    start: u32,
    end: u32,
}
/// Native-independent style extent in the exact UTF-16 document character space.
#[derive(Clone, Copy)]
pub(crate) struct StyleExtent {
    pub(crate) start: i32,
    pub(crate) end: i32,
    pub(crate) style: TextStyle,
}
/// The index and copied native document share one original pre-effect claim.
/// Native custody keeps this owner until checked document/window retirement.
pub(crate) struct DocumentIndex {
    rows: Vec<Row>,
    text: Vec<u16>,
    styles: Vec<StyleExtent>,
    pub(crate) resource: Option<ResourceKey>,
    pub(crate) revision: u64,
    pub(crate) units: u32,
    _claim: MemoryClaim,
}
impl DocumentIndex {
    /// Admit an empty native document without fabricating an output resource identity.
    pub(crate) fn empty(claim: MemoryClaim) -> Result<Self, DesktopError> {
        if claim.bytes() < NATIVE_CONSOLE_STORAGE_BYTES {
            return Err(rejected());
        }
        Ok(Self {
            rows: Vec::new(),
            text: Vec::new(),
            styles: Vec::new(),
            resource: None,
            revision: 0,
            units: 0,
            _claim: claim,
        })
    }
    /// Validate exact extent, text and increasing identities before native text effects.
    pub(crate) fn prepare(
        source: &dyn ConsoleText,
        claim: MemoryClaim,
    ) -> Result<Self, DesktopError> {
        if claim.bytes() < NATIVE_CONSOLE_STORAGE_BYTES || source.len() > MAX_SNAPSHOT_RECORDS {
            return Err(rejected());
        }
        let mut rows = Vec::new();
        rows.try_reserve_exact(source.len())
            .map_err(|e| DesktopError::new("console_index_allocate", e))?;
        if rows.capacity() != source.len() {
            return Err(rejected());
        }
        let mut bytes = 0usize;
        let mut units = 0u32;
        let mut previous = None;
        let mut run_count = 0usize;
        for index in 0..source.len() {
            let record = source.record(index).ok_or_else(rejected)?;
            bytes = bytes.checked_add(record.text().len()).ok_or_else(rejected)?;
            if bytes > MAX_SNAPSHOT_BYTES
                || previous.is_some_and(|id| id >= record.id())
                || record.text().contains(['\r', '\n'])
            {
                return Err(rejected());
            }
            run_count = run_count.checked_add(record.runs().len()).ok_or_else(rejected)?;
            if run_count > MAX_STYLE_RUNS {
                return Err(rejected());
            }
            if index != 0 {
                units += 1;
            }
            let start = units;
            units += record.text().encode_utf16().count() as u32;
            rows.push(Row { id: record.id(), start, end: units });
            previous = Some(record.id());
        }
        if source.record(source.len()).is_some() {
            return Err(rejected());
        }
        let mut text = Vec::new();
        text.try_reserve_exact(units as usize)
            .map_err(|e| DesktopError::new("console_text_allocate", e))?;
        let mut styles: Vec<StyleExtent> = Vec::new();
        styles
            .try_reserve_exact(run_count)
            .map_err(|e| DesktopError::new("console_styles_allocate", e))?;
        if text.capacity() != units as usize || styles.capacity() != run_count {
            return Err(rejected());
        }
        for (index, row) in rows.iter().enumerate() {
            let record = source.record(index).ok_or_else(rejected)?;
            if record.id() != row.id
                || record.text().encode_utf16().count() as u32 != row.end - row.start
                || text.len() + usize::from(index != 0) + (row.end - row.start) as usize
                    > text.capacity()
                || styles.len() + record.runs().len() > styles.capacity()
            {
                return Err(rejected());
            }
            if index != 0 {
                text.push(13);
            }
            text.extend(record.text().encode_utf16());
            let mut position = row.start;
            for run in record.runs() {
                let next = position
                    + record.text()[run.start as usize..run.end as usize].encode_utf16().count()
                        as u32;
                if let Some(previous) = styles.last_mut()
                    && previous.style == run.style
                    && (previous.end == position as i32
                        || previous.end + 1 == position as i32
                            && run.style.background == Default::default())
                {
                    // Equal foreground runs can include a paragraph separator. An explicit
                    // background must not paint that separator with another record's color.
                    previous.end = next as i32;
                } else {
                    styles.push(StyleExtent {
                        start: position as i32,
                        end: next as i32,
                        style: run.style,
                    });
                }
                position = next;
            }
        }
        Ok(Self {
            rows,
            text,
            styles,
            resource: Some(source.resource()),
            revision: source.revision(),
            units,
            _claim: claim,
        })
    }
    /// Translate a checked native position to a stable record-local selection anchor.
    pub(crate) fn anchor(&self, position: i32) -> Option<Anchor> {
        let position = position.max(0) as u32;
        let row = self.rows.iter().rev().find(|row| row.start <= position).or(self.rows.first())?;
        Some(Anchor { record: row.id, units: position.min(row.end).saturating_sub(row.start) })
    }
    /// Rebase only within the same resource. Trimmed anchors select the first surviving
    /// successor; a replaced shorter line clamps at a Unicode scalar boundary.
    pub(crate) fn position(&self, anchor: Option<Anchor>) -> i32 {
        let Some(anchor) = anchor else { return 0 };
        let Some(row) = self.rows.iter().find(|row| row.id >= anchor.record) else {
            return self.units as i32;
        };
        if row.id != anchor.record {
            return row.start as i32;
        }
        let mut position = row.start + anchor.units.min(row.end - row.start);
        if position > row.start
            && position < row.end
            && (0xdc00..=0xdfff).contains(&self.text[position as usize])
            && (0xd800..=0xdbff).contains(&self.text[position as usize - 1])
        {
            position -= 1;
        }
        position as i32
    }
}
fn rejected() -> DesktopError {
    DesktopError::message("console_index_validate", "invalid bounded immutable console view")
}
/// Stream already prepared exact UTF-16 without consulting a provider inside Win32.
pub(crate) struct TextStream<'a> {
    units: &'a [u16],
    offset: usize,
}
impl<'a> TextStream<'a> {
    pub(crate) fn new(document: &'a DocumentIndex) -> Self {
        Self { units: &document.text, offset: 0 }
    }
    /// Stream an already validated native edit slice, including an empty deletion.
    #[cfg(target_os = "windows")]
    pub(crate) fn slice(units: &'a [u16]) -> Self {
        Self { units, offset: 0 }
    }
    /// Preserve exact UTF-16 bytes even when a callback splits a storage unit or surrogate pair.
    pub(crate) fn read(&mut self, target: &mut [u8]) -> usize {
        let mut written = 0;
        for byte in target {
            let Some(unit) = self.units.get(self.offset / 2) else {
                break;
            };
            *byte = unit.to_le_bytes()[self.offset % 2];
            self.offset += 1;
            written += 1;
        }
        written
    }
    pub(crate) fn complete(&self) -> bool {
        self.offset == self.units.len() * 2
    }
}
impl DocumentIndex {
    /// Borrow prepared style ranges in the same native UTF-16 character space.
    pub(crate) fn styles(&self) -> &[StyleExtent] {
        &self.styles
    }
    /// Borrow exact UTF-16 including foreign control scalars and record delimiters.
    pub(crate) fn text(&self) -> &[u16] {
        &self.text
    }
}
const _: () = assert!(
    6 * (MAX_SNAPSHOT_BYTES + MAX_SNAPSHOT_RECORDS)
        + MAX_SNAPSHOT_RECORDS * size_of::<Row>()
        + MAX_STYLE_RUNS * size_of::<StyleExtent>()
        + 16 * 1024
        < NATIVE_CONSOLE_STORAGE_BYTES as usize
);
#[cfg(test)]
pub(crate) mod tests;
