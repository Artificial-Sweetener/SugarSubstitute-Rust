// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transpose bounded scanline stripes into sequential hashed chunks through shared storage.
use crate::{ImportError, RasterDescriptor, buffer::Buffer};
use sha2::{Digest, Sha256};
use std::io::{self, Write};
use sugarsubstitute_contracts::execution::JobContext;

/// Encode an archive from exact top-to-bottom straight RGBA scanlines.
/// The callback fills one admitted row in the descriptor's integer sample representation.
/// This operation packs original decoded samples; image processing belongs to the DAG.
/// At most one 8 MiB stripe and one 128x128 chunk are retained, regardless of source height.
/// The storage owner grants and stages the output; this function opens no files or cache.
/// Partial writes and transient interruptions retain the original job's cancellation/deadline.
/// # Errors
/// Preserves decoder, storage, allocation, admission and control failures. The caller must
/// never publish an incomplete output stream as a complete cache entry.
pub fn write_raster(
    output: &mut dyn Write,
    descriptor: RasterDescriptor,
    context: &JobContext,
    mut row: impl FnMut(u32, &mut [u8]) -> Result<(), ImportError>,
) -> Result<(), ImportError> {
    crate::check(context)?;
    let layout = descriptor.layout();
    let stride = layout.packing().bytes() as usize;
    let row_bytes = layout.extent().width() as usize * stride;
    let mut stripe = Buffer::new(layout.stripe_bytes(), 0, context.memory())?;
    let mut chunk =
        Buffer::new(layout.region(0, 0)?.pixels() as usize * stride, 0, context.memory())?;
    write_complete(output, &descriptor.header(), context)?;
    for index in 0..layout.stripes() {
        let first = layout.region(0, index)?;
        for offset in 0..first.height() {
            crate::check(context)?;
            let start = offset as usize * row_bytes;
            row(first.y() + offset, &mut stripe.data[start..start + row_bytes])?;
        }
        for column in 0..layout.columns() {
            crate::check(context)?;
            let region = layout.region(column, index)?;
            let width = region.width() as usize * stride;
            for offset in 0..region.height() as usize {
                let start = offset * row_bytes + region.x() as usize * stride;
                chunk.data[offset * width..(offset + 1) * width]
                    .copy_from_slice(&stripe.data[start..start + width]);
            }
            let data = &chunk.data[..region.pixels() as usize * stride];
            let hash: [u8; 32] = Sha256::digest(data).into();
            write_complete(output, &hash, context)?;
            write_complete(output, data, context)?;
        }
    }
    crate::check(context)
}

/// Finish one sequential span while checking controls before every interrupted-write retry.
/// Native failures keep their original causes; exhausted writers retain standard WriteZero.
fn write_complete(
    output: &mut dyn Write,
    mut bytes: &[u8],
    context: &JobContext,
) -> Result<(), ImportError> {
    while !bytes.is_empty() {
        crate::check(context)?;
        match output.write(bytes) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "Image archive output stopped before the complete span",
                )
                .into());
            }
            Ok(count) => bytes = &bytes[count..],
            Err(cause) if cause.kind() == io::ErrorKind::Interrupted => {}
            Err(cause) => return Err(cause.into()),
        }
    }
    crate::check(context)
}
