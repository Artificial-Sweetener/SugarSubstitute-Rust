// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Merge bounded DAG sampling plans into source-stripe-ordered authenticated reads.
use super::*;
use sugarsubstitute_image_ops::{MAX_RASTER_SAMPLE_BATCH, RasterSampleRequest};

struct SparseBatch {
    first_column: u32,
    last_column: u32,
    bytes: usize,
}

impl<R: Read + Seek> RasterArchive<R> {
    pub(super) fn read_sparse_batch(
        &mut self,
        requests: &mut [RasterSampleRequest<'_>],
        context: &JobContext,
    ) -> Result<(), ImportError> {
        let layout = self.descriptor.layout();
        let stride = layout.packing().bytes() as usize;
        let mut row_top = [0_usize; MAX_RASTER_SAMPLE_BATCH];
        loop {
            let Some(stripe) = requests
                .iter()
                .enumerate()
                .filter_map(|(index, request)| {
                    request.samples.rows().get(row_top[index]).map(|row| row / layout.rows())
                })
                .min()
            else {
                return Ok(());
            };
            let mut row_bottom = row_top;
            for (index, request) in requests.iter().enumerate() {
                if request
                    .samples
                    .rows()
                    .get(row_top[index])
                    .is_some_and(|row| row / layout.rows() == stripe)
                {
                    row_bottom[index] += request.samples.rows()[row_top[index]..]
                        .partition_point(|row| row / layout.rows() == stripe);
                }
            }
            let mut columns = [0_usize; MAX_RASTER_SAMPLE_BATCH];
            while let Some(first_column) = next_column(requests, &columns, &row_top, &row_bottom) {
                let starts = columns;
                let batch = self.sparse_batch(
                    requests,
                    &mut columns,
                    &row_top,
                    &row_bottom,
                    (stripe, first_column),
                )?;
                crate::check(context)?;
                self.read_encoded(layout.offset(batch.first_column, stripe)?, batch.bytes)?;
                self.stats.read_bytes = self.stats.read_bytes.saturating_add(batch.bytes as u64);
                crate::check(context)?;
                let mut encoded = 0;
                let mut samples = starts;
                for column in batch.first_column..=batch.last_column {
                    crate::check(context)?;
                    let chunk = layout.region(column, stripe)?;
                    let length = chunk.pixels() as usize * stride;
                    let expected: [u8; 32] = self.encoded.data[encoded..encoded + 32]
                        .try_into()
                        .map_err(|_| ImportError::Invalid("Source chunk checksum width changed"))?;
                    let payload = encoded + 32..encoded + 32 + length;
                    self.stats.chunks = self.stats.chunks.saturating_add(1);
                    let actual: [u8; 32] =
                        Sha256::digest(&self.encoded.data[payload.clone()]).into();
                    if actual != expected {
                        return Err(ImportError::Corrupt(Box::new(ImportError::Invalid(
                            "Reconstructed source chunk failed integrity verification",
                        ))));
                    }
                    for (index, request) in requests.iter_mut().enumerate() {
                        if row_top[index] == row_bottom[index] {
                            continue;
                        }
                        let right = samples[index]
                            + request.samples.columns()[samples[index]..columns[index]]
                                .partition_point(|sample| sample / TILE_EDGE == column);
                        for y in row_top[index]..row_bottom[index] {
                            crate::check(context)?;
                            for sample in samples[index]..right {
                                let source = ((request.samples.rows()[y] - chunk.y()) as usize
                                    * chunk.width() as usize
                                    + (request.samples.columns()[sample] - chunk.x()) as usize)
                                    * stride
                                    + payload.start;
                                request.output[y * request.samples.columns().len() + sample] =
                                    self.decode_pixel(&self.encoded.data[source..source + stride])?;
                            }
                        }
                        samples[index] = right;
                    }
                    encoded = payload.end;
                }
                if encoded != batch.bytes || samples[..requests.len()] != columns[..requests.len()]
                {
                    return Err(ImportError::Invalid("Sparse source batch layout differs"));
                }
            }
            row_top = row_bottom;
        }
    }

    fn sparse_batch(
        &self,
        requests: &[RasterSampleRequest<'_>],
        columns: &mut [usize; MAX_RASTER_SAMPLE_BATCH],
        row_top: &[usize; MAX_RASTER_SAMPLE_BATCH],
        row_bottom: &[usize; MAX_RASTER_SAMPLE_BATCH],
        start: (u32, u32),
    ) -> Result<SparseBatch, ImportError> {
        let (stripe, first_column) = start;
        let layout = self.descriptor.layout();
        let stride = layout.packing().bytes() as usize;
        let mut last_column = first_column;
        let mut bytes = 0_usize;
        loop {
            let Some(column) = next_column(requests, columns, row_top, row_bottom) else {
                break;
            };
            if bytes != 0 && last_column.checked_add(1) != Some(column) {
                break;
            }
            let chunk = layout.region(column, stripe)?;
            let length = (chunk.pixels() as usize)
                .checked_mul(stride)
                .and_then(|length| length.checked_add(32))
                .ok_or(ImportError::Invalid("Sparse source batch size overflow"))?;
            let next = bytes
                .checked_add(length)
                .ok_or(ImportError::Invalid("Sparse source batch size overflow"))?;
            if next > self.encoded.data.len() {
                if bytes == 0 {
                    return Err(ImportError::Invalid(
                        "Sparse source chunk exceeds bounded workspace",
                    ));
                }
                break;
            }
            bytes = next;
            last_column = column;
            for (index, request) in requests.iter().enumerate() {
                if row_top[index] != row_bottom[index]
                    && request
                        .samples
                        .columns()
                        .get(columns[index])
                        .is_some_and(|sample| sample / TILE_EDGE == column)
                {
                    columns[index] += request.samples.columns()[columns[index]..]
                        .partition_point(|sample| sample / TILE_EDGE == column);
                }
            }
        }
        Ok(SparseBatch { first_column, last_column, bytes })
    }
}

fn next_column(
    requests: &[RasterSampleRequest<'_>],
    columns: &[usize; MAX_RASTER_SAMPLE_BATCH],
    row_top: &[usize; MAX_RASTER_SAMPLE_BATCH],
    row_bottom: &[usize; MAX_RASTER_SAMPLE_BATCH],
) -> Option<u32> {
    requests
        .iter()
        .enumerate()
        .filter(|(index, _)| row_top[*index] != row_bottom[*index])
        .filter_map(|(index, request)| request.samples.columns().get(columns[index]))
        .map(|sample| sample / TILE_EDGE)
        .min()
}
