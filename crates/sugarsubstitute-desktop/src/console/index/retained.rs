// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Identify unchanged retained records before applying bounded native tail edits.
use super::*;

/// Old leading characters to discard and unchanged incoming characters to preserve.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Retained {
    pub(crate) trim: u32,
    pub(crate) units: u32,
}
impl DocumentIndex {
    /// Reuse only matching record identities, exact Unicode and semantic styles.
    /// Current-line rewrites or foreign resources end retention before native edits.
    pub(crate) fn retained(&self, previous: Option<&Self>) -> Retained {
        let Some(old) = previous.filter(|old| old.resource == self.resource) else {
            return Retained::default();
        };
        let Some(first) = self.rows.first() else { return Retained::default() };
        let Some(index) = old.rows.iter().position(|row| row.id == first.id) else {
            return Retained::default();
        };
        let trim = old.rows[index].start;
        let mut units = 0;
        for (incoming, previous) in self.rows.iter().zip(&old.rows[index..]) {
            if incoming.id != previous.id
                || self.text[incoming.start as usize..incoming.end as usize]
                    != old.text[previous.start as usize..previous.end as usize]
                || !self.row_styles(incoming).eq(old.row_styles(previous))
            {
                break;
            }
            units = incoming.end;
        }
        if units == 0 { Retained::default() } else { Retained { trim, units } }
    }
    fn row_styles<'a>(&'a self, row: &'a Row) -> impl Iterator<Item = (i32, i32, TextStyle)> + 'a {
        self.styles.iter().filter_map(move |extent| {
            let start = extent.start.max(row.start as i32);
            let end = extent.end.min(row.end as i32);
            (start < end).then_some((
                start - row.start as i32,
                end - row.start as i32,
                extent.style,
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{TestResult, index, source};
    use super::*;
    #[test]
    fn retention_requires_identity_unicode_and_style_and_stops_at_current_line_rewrite()
    -> TestResult {
        let (old, _, _) = index(&source(1, &[(1, "gone"), (2, "日本😀"), (3, "waiting")])?)?;
        let (same, _, _) = index(&source(2, &[(2, "日本😀"), (3, "waiting"), (4, "new")])?)?;
        assert_eq!(same.retained(Some(&old)), Retained { trim: 5, units: 12 });
        let (changed, _, _) = index(&source(3, &[(2, "日本😀"), (3, "ready")])?)?;
        assert_eq!(changed.retained(Some(&old)), Retained { trim: 5, units: 4 });
        let (foreign, _, _) = index(&source(4, &[(5, "日本😀")])?)?;
        assert_eq!(foreign.retained(Some(&old)), Retained::default());
        let mut styled = source(5, &[(2, "日本😀")])?;
        styled.records[0] = TranscriptRecord::new(
            RecordId::new(2)?,
            OutputStream::Stdout,
            "日本😀".to_owned(),
            vec![StyleRun { start: 0, end: 10, style: TextStyle::default() }],
            true,
        )?;
        let (styled, _, _) = index(&styled)?;
        assert_eq!(styled.retained(Some(&old)), Retained::default());
        Ok(())
    }
}
