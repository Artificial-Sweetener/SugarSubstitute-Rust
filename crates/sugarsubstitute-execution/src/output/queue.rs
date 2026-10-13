// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep exact interleaved loss before the next retained same-stream sequence.
use super::*;
impl Loss {
    fn add(&mut self, other: Self) -> Result<(), NativeObservationError> {
        let bytes =
            self.bytes.checked_add(other.bytes).ok_or_else(|| invariant("output_loss_overflow"))?;
        let batches = self
            .batches
            .checked_add(other.batches)
            .ok_or_else(|| invariant("output_loss_overflow"))?;
        self.bytes = bytes;
        self.batches = batches;
        Ok(())
    }
    pub(super) fn gap(self) -> Result<Option<OutputGap>, WindowsOutputError> {
        if self.bytes == 0 {
            Ok(None)
        } else {
            OutputGap::new(self.bytes, self.batches)
                .map(Some)
                .map_err(|_| WindowsOutputError::Exhausted)
        }
    }
}
impl State {
    fn next(&mut self, stream: OutputStream) -> Result<u64, NativeObservationError> {
        let value = self.sequences[stream.index()]
            .checked_add(1)
            .ok_or_else(|| invariant("output_sequence_exhausted"))?;
        self.sequences[stream.index()] = value;
        Ok(value)
    }
    fn discard_front(&mut self) -> Result<bool, NativeObservationError> {
        let Some(chunk) = self.queue.front() else { return Ok(false) };
        let size = chunk.buffer.bytes.len();
        let loss = Loss { bytes: size as u64, batches: 1 };
        let index = chunk.stream.index();
        let dropped_bytes = self.counters[index]
            .dropped_bytes
            .checked_add(size as u64)
            .ok_or_else(|| invariant("output_counter_overflow"))?;
        let dropped_batches = self.counters[index]
            .dropped_batches
            .checked_add(1)
            .ok_or_else(|| invariant("output_counter_overflow"))?;
        let mut merged = chunk.gap;
        merged.add(loss)?;
        let next = self.queue.iter().skip(1).position(|c| c.stream == chunk.stream).map(|n| n + 1);
        let mut destination = next.map_or(self.tail[index], |n| self.queue[n].gap);
        destination.add(merged)?;
        self.counters[index].dropped_bytes = dropped_bytes;
        self.counters[index].dropped_batches = dropped_batches;
        if let Some(next) = next {
            self.queue[next].gap = destination;
        } else {
            self.tail[index] = destination;
        }
        self.queued_bytes -= size;
        let chunk = self.queue.pop_front();
        drop(chunk);
        Ok(true)
    }
    pub(super) fn push(
        &mut self,
        stream: OutputStream,
        bytes: &[u8],
    ) -> Result<(), NativeObservationError> {
        if bytes.is_empty()
            || bytes.len() > MAX_BATCH_BYTES
            || self.eof_sequence[stream.index()].is_some()
        {
            return Err(invariant("output_read_shape"));
        }
        let index = stream.index();
        let read_bytes = self.counters[index]
            .read_bytes
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| invariant("output_counter_overflow"))?;
        let sequence = self.next(stream)?;
        self.counters[index].read_bytes = read_bytes;
        while self.queue.len() >= MAX_WINDOWS_OUTPUT_CHUNKS
            || self.queued_bytes + bytes.len() > MAX_WINDOWS_OUTPUT_QUEUED_BYTES
        {
            self.discard_front()?;
        }
        let memory = self.memory.clone().ok_or_else(|| invariant("output_missing_memory"))?;
        let buffer = loop {
            match Buffer::new(&memory, bytes) {
                Ok(buffer) => break Some(buffer),
                Err(cause) if cause.stage() == "output_chunk_claim" => {
                    if !self.discard_front()? {
                        break None;
                    }
                }
                Err(cause) => return Err(cause),
            }
        };
        if let Some(buffer) = buffer {
            self.queued_bytes += bytes.len();
            let gap = std::mem::take(&mut self.tail[index]);
            self.queue.push_back(Chunk { stream, sequence, buffer, gap });
        } else {
            let mut tail = self.tail[index];
            tail.add(Loss { bytes: bytes.len() as u64, batches: 1 })?;
            let dropped_bytes = self.counters[index]
                .dropped_bytes
                .checked_add(bytes.len() as u64)
                .ok_or_else(|| invariant("output_counter_overflow"))?;
            let dropped_batches = self.counters[index]
                .dropped_batches
                .checked_add(1)
                .ok_or_else(|| invariant("output_counter_overflow"))?;
            self.tail[index] = tail;
            self.counters[index].dropped_bytes = dropped_bytes;
            self.counters[index].dropped_batches = dropped_batches;
        }
        Ok(())
    }
    pub(super) fn eof(&mut self, stream: OutputStream) -> Result<(), NativeObservationError> {
        if self.eof_sequence[stream.index()].is_none() {
            let sequence = self.next(stream)?;
            self.eof_sequence[stream.index()] = Some(sequence);
        }
        Ok(())
    }
    pub(super) fn batch(
        &mut self,
        identity: OutputIdentity,
    ) -> Result<Option<(Batch, MemoryClaim)>, WindowsOutputError> {
        let memory = self.memory.as_ref().ok_or(WindowsOutputError::NotReady)?;
        let claim =
            memory.claim(MAX_BATCH_BYTES as u64).map_err(|_| WindowsOutputError::MemoryBusy)?;
        if let Some(chunk) = self.queue.front() {
            let batch = Batch::new(
                identity,
                chunk.stream,
                StreamSequence::new(chunk.sequence).map_err(|_| WindowsOutputError::Exhausted)?,
                &chunk.buffer.bytes,
                chunk.gap.gap()?,
                false,
            )
            .map_err(|_| WindowsOutputError::Exhausted)?;
            if let Some(chunk) = self.queue.pop_front() {
                self.queued_bytes -= chunk.buffer.bytes.len();
                drop(chunk);
            }
            return Ok(Some((batch, claim)));
        }
        for stream in [OutputStream::Stdout, OutputStream::Stderr] {
            let i = stream.index();
            if self.eof_delivered[i] || self.gap_delivered[i] {
                continue;
            }
            let end = self.eof_sequence[i].is_some();
            if !(end || self.retired && self.tail[i].bytes != 0) {
                continue;
            }
            let sequence = match self.eof_sequence[i] {
                Some(value) => value,
                None => self.next(stream).map_err(|_| WindowsOutputError::Exhausted)?,
            };
            let batch = Batch::new(
                identity,
                stream,
                StreamSequence::new(sequence).map_err(|_| WindowsOutputError::Exhausted)?,
                &[],
                self.tail[i].gap()?,
                end,
            )
            .map_err(|_| WindowsOutputError::Exhausted)?;
            self.tail[i] = Loss::default();
            if end {
                self.eof_delivered[i] = true;
            } else {
                self.gap_delivered[i] = true;
            }
            return Ok(Some((batch, claim)));
        }
        Ok(None)
    }
}
