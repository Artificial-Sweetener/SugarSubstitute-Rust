// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish bounded current records and evict oldest history without whole-history copies.
use super::*;
impl TranscriptStore {
    pub(super) fn ensure_id(&mut self, index: usize) -> Result<RecordId, TranscriptError> {
        if let Some(id) = self.streams[index].line.id {
            return Ok(id);
        }
        let id = RecordId::new(self.next_id)?;
        self.next_id += 1;
        self.streams[index].line.id = Some(id);
        self.streams[index].line.published = false;
        Ok(id)
    }
    pub(super) fn character(
        &mut self,
        index: usize,
        character: char,
        change: &mut TranscriptChange,
    ) -> Result<(), TranscriptError> {
        if character == '\n' {
            return self.finish_line(index, true, change);
        }
        if character == '\r' {
            self.streams[index].line.cr = true;
            return Ok(());
        }
        self.ensure_id(index)?;
        let style = self.streams[index].parser.style();
        let line = &mut self.streams[index].line;
        if line.cr {
            line.clear();
        }
        let new_run = line.runs.last().is_none_or(|run| run.style != style);
        let over = line.text.len() + character.len_utf8() > MAX_RECORD_BYTES
            || (new_run && line.runs.len() == MAX_RECORD_RUNS);
        if line.truncated || over {
            let newly = !line.truncated;
            line.truncated = true;
            self.add(Counter::TruncatedBytes, character.len_utf8() as u64);
            if newly {
                self.add(Counter::TruncatedRecords, 1);
            }
            return Ok(());
        }
        let start = line.text.len() as u32;
        line.text.push(character);
        if new_run {
            line.runs.push(StyleRun { start, end: line.text.len() as u32, style });
        } else if let Some(run) = line.runs.last_mut() {
            run.end = line.text.len() as u32;
        }
        Ok(())
    }
    pub(super) fn finish_line(
        &mut self,
        index: usize,
        empty: bool,
        change: &mut TranscriptChange,
    ) -> Result<(), TranscriptError> {
        if self.streams[index].line.id.is_some() || empty {
            self.ensure_id(index)?;
            self.publish(index, true, change)?;
        }
        let line = &mut self.streams[index].line;
        line.id = None;
        line.published = false;
        line.clear();
        Ok(())
    }
    pub(super) fn publish(
        &mut self,
        index: usize,
        complete: bool,
        change: &mut TranscriptChange,
    ) -> Result<(), TranscriptError> {
        let line = &self.streams[index].line;
        let Some(id) = line.id else { return Ok(()) };
        let stream = if index == 0 { OutputStream::Stdout } else { OutputStream::Stderr };
        let record = Arc::new(TranscriptRecord::new(
            id,
            stream,
            line.text.clone(),
            line.runs.clone(),
            complete,
        )?);
        let position =
            if line.published { self.records.iter().position(|r| r.id() == id) } else { None };
        if let Some(position) = position {
            let old = &self.records[position];
            self.text_bytes -= old.text().len();
            self.style_runs -= old.runs().len();
            self.records[position] = record.clone();
        } else {
            // Evict before push so the fixed2000-slot container never doubles.
            if self.records.len() == MAX_RECORDS {
                self.trim(change);
            }
            self.records.push_back(record.clone());
        }
        self.text_bytes += record.text().len();
        self.style_runs += record.runs().len();
        while self.text_bytes > MAX_TEXT_BYTES || self.style_runs > MAX_STYLE_RUNS {
            self.trim(change);
        }
        if self.streams[index].line.id == Some(id) {
            self.streams[index].line.published = true;
        }
        Ok(())
    }
    fn trim(&mut self, change: &mut TranscriptChange) {
        if let Some(record) = self.records.pop_front() {
            self.text_bytes -= record.text().len();
            self.style_runs -= record.runs().len();
            change.trimmed_through = Some(record.id());
            self.add(Counter::TrimmedRecords, 1);
            self.add(Counter::TrimmedBytes, record.text().len() as u64);
            for stream in &mut self.streams {
                if stream.line.id == Some(record.id()) {
                    stream.line.id = None;
                    stream.line.published = false;
                    stream.line.clear();
                }
            }
        }
    }
}
