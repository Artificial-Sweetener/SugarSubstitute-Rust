// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Headless driver-boundary fixtures using the production display shader and mapping upload.
#[path = "boundary_fixtures/patches.rs"]
mod patches;
use crate::{
    DisplayImage, DisplayMapping, GpuError, OverlayPixel,
    display::row_bytes,
    display_mapping::MappingUpload,
    display_readback::{DisplayReadbackWorkspace, read_texture_region_into},
    transfer::Transfer,
    window_overlay::OverlayBatches,
    window_shader::Blitter,
};
pub use patches::OffscreenDisplay;
use std::sync::Arc;
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_ops::{DisplaySample, bilinear_display_pixel, is_premultiplied_word};

/// One retained source, its production mapping, and independently encoded CPU reference pixels.
///
/// `columns` and `rows` must be the exact arrays used to construct `mapping`. This feature-gated
/// boundary descriptor keeps the production mapping type opaque while CPU reference interpolation
/// remains independent of shader execution.
pub struct CompositeSource<'a> {
    image: DisplayImage,
    mapping: DisplayMapping<'a>,
    pixels: &'a [u32],
    columns: &'a [DisplaySample],
    rows: &'a [DisplaySample],
}

impl<'a> CompositeSource<'a> {
    /// Bind one immutable display image to its mapping and premultiplied `0xAARRGGBB` CPU
    /// reference.
    /// # Errors
    /// Rejects mismatched extents, incomplete maps, invalid indices and non-premultiplied
    /// reference pixels.
    pub fn new(
        image: DisplayImage,
        mapping: DisplayMapping<'a>,
        pixels: &'a [u32],
        columns: &'a [DisplaySample],
        rows: &'a [DisplaySample],
    ) -> Result<Self, GpuError> {
        if image.extent() != mapping.source()
            || pixels.len() as u64 != image.extent().pixels()
            || columns.len() != mapping.output().width() as usize
            || rows.len() != mapping.output().height() as usize
            || columns.iter().any(|sample| !sample.is_valid_for(image.extent().width()))
            || rows.iter().any(|sample| !sample.is_valid_for(image.extent().height()))
            || !pixels.iter().all(|pixel| is_premultiplied_word(*pixel))
        {
            return Err(GpuError::invalid("Invalid composite qualification source"));
        }
        Ok(Self { image, mapping, pixels, columns, rows })
    }
}

/// Complete primary/fallback projection tested through one actual selected driver.
pub struct CompositeFixture<'a> {
    primary: CompositeSource<'a>,
    fallback: CompositeSource<'a>,
    matte: [u8; 4],
    overlay: &'a [OverlayPixel],
}

impl<'a> CompositeFixture<'a> {
    /// Validate one same-device mapping pair and its background before native effects.
    /// # Errors
    /// Rejects foreign devices, differing outputs or a matte that is neither opaque nor
    /// transparent black.
    pub fn new(
        primary: CompositeSource<'a>,
        fallback: CompositeSource<'a>,
        matte: [u8; 4],
    ) -> Result<Self, GpuError> {
        if !Arc::ptr_eq(primary.image.device(), fallback.image.device())
            || primary.mapping.output() != fallback.mapping.output()
            || !matches!(matte, [_, _, _, 255] | [0, 0, 0, 0])
        {
            return Err(GpuError::invalid("Invalid composite qualification fixture"));
        }
        Ok(Self { primary, fallback, matte, overlay: &[] })
    }
    /// Draw these opaque overlay pixels over the composite, later pixels over earlier ones.
    pub const fn with_overlay(mut self, overlay: &'a [OverlayPixel]) -> Self {
        self.overlay = overlay;
        self
    }
}

/// Exact result of one offscreen production-shader qualification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositeQualification {
    /// Physical pixels compared exactly against the CPU mapping reference.
    pub pixels: u64,
    /// Stable FNV-1a hash of the GPU `0xffRRGGBB` words in row-major order.
    pub pixel_hash: u64,
}

/// Run the production native blitter against an offscreen `Rgba8Unorm` render attachment.
///
/// The caller must invoke this on an admitted worker. All output, upload, command and readback
/// storage is admitted before command encoding; submission and mapping retire before return.
/// # Errors
/// Preserves control, admission and driver failures and reports the first exact pixel mismatch.
pub fn qualify_composite(
    fixture: CompositeFixture<'_>,
    context: &JobContext,
) -> Result<CompositeQualification, GpuError> {
    crate::adapter::check(context)?;
    let output = fixture.primary.mapping.output();
    if output.width() > 128 || output.height() > 128 {
        return Err(GpuError::invalid("Composite qualification output exceeds one readback tile"));
    }
    let device = fixture.primary.image.device().clone();
    let blitter = Blitter::new(&device, wgpu::TextureFormat::Rgba8Unorm, context)?;
    let upload = MappingUpload::new(
        Some(fixture.primary.mapping),
        Some(fixture.fallback.mapping),
        output,
        context,
    )?;
    let overlay = OverlayBatches::upload(&device, fixture.overlay, output, context)?;
    let mapping = blitter.mapping();
    let staging = context
        .memory()
        .claim(upload.bytes.len() as u64 + 128)
        .map_err(|cause| GpuError::new("gpu_composite_mapping_staging", cause))?;
    let mut readback =
        DisplayReadbackWorkspace::for_region(&device, output, output.region(), context)?;
    let _output_memory = device
        .memory
        .claim(
            (u64::from(row_bytes(output.width())) * u64::from(output.height()))
                .next_multiple_of(65536)
                + 4096,
        )
        .map_err(|cause| GpuError::new("gpu_composite_output_admission", cause))?;
    let _command_memory = context
        .memory()
        .claim(4096)
        .map_err(|cause| GpuError::new("gpu_composite_command_admission", cause))?;
    crate::adapter::check(context)?;
    device.faults.check()?;
    let texture = device.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("SugarSubstitute offscreen composite qualification"),
        size: wgpu::Extent3d {
            width: output.width(),
            height: output.height(),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let output_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let primary_view = fixture.primary.image.view();
    let fallback_view = fixture.fallback.image.view();
    let mut commands = device.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("SugarSubstitute offscreen composite commands"),
    });
    blitter.encode(
        &device,
        &primary_view,
        &fallback_view,
        &output_view,
        &overlay,
        &[],
        &mut commands,
    );
    device.faults.check()?;
    let submission = device.submission(context)?;
    let mut transfer =
        Transfer::upload(mapping, staging, &upload.bytes, Some(commands), submission);
    transfer.retire()?;
    crate::adapter::check(context)?;
    read_texture_region_into(&texture, &device, output, output.region(), &mut readback, context)?;
    readback.swap_red_blue(output.pixels() as usize);
    let actual = readback.pixels(output.pixels() as usize);
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for (index, &pixel) in actual.iter().enumerate() {
        let x = index % output.width() as usize;
        let y = index / output.width() as usize;
        let expected = expected_pixel(&fixture, x, y);
        if pixel != expected {
            return Err(GpuError::new(
                "gpu_composite_pixel",
                std::io::Error::other(format!(
                    "Offscreen composite differs at pixel {index}: \
                     actual={pixel:#010x}, expected={expected:#010x}"
                )),
            ));
        }
        for byte in pixel.to_be_bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    Ok(CompositeQualification { pixels: output.pixels(), pixel_hash: hash })
}

fn expected_pixel(fixture: &CompositeFixture<'_>, x: usize, y: usize) -> u32 {
    if let Some(pixel) =
        fixture.overlay.iter().rev().find(|pixel| (pixel.x as usize, pixel.y as usize) == (x, y))
    {
        let [red, green, blue] = pixel.color;
        return u32::from_be_bytes([255, red, green, blue]);
    }
    sample(&fixture.primary, x, y).or_else(|| sample(&fixture.fallback, x, y)).unwrap_or_else(
        || {
            u32::from_be_bytes([
                fixture.matte[3],
                fixture.matte[0],
                fixture.matte[1],
                fixture.matte[2],
            ])
        },
    )
}

fn sample(source: &CompositeSource<'_>, x: usize, y: usize) -> Option<u32> {
    let horizontal = source.columns[x];
    let vertical = source.rows[y];
    if !horizontal.is_covered() || !vertical.is_covered() {
        return None;
    }
    let width = source.image.extent().width() as usize;
    let at = |column: u32, row: u32| source.pixels[row as usize * width + column as usize];
    Some(bilinear_display_pixel(
        [
            at(horizontal.first(), vertical.first()),
            at(horizontal.second(), vertical.first()),
            at(horizontal.first(), vertical.second()),
            at(horizontal.second(), vertical.second()),
        ],
        horizontal,
        vertical,
    ))
}
