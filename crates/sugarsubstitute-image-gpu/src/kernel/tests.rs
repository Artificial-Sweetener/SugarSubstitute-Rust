// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate actual shader syntax and portable capabilities without requiring a native GPU.
use super::*;

#[test]
fn shared_image_shader_parses_and_validates_without_optional_device_capabilities()
-> Result<(), GpuError> {
    let module = wgpu::naga::front::wgsl::parse_str(SOURCE)
        .map_err(|cause| GpuError::new("image_shader_parse", cause))?;
    wgpu::naga::valid::Validator::new(
        wgpu::naga::valid::ValidationFlags::all(),
        wgpu::naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .map_err(|cause| GpuError::new("image_shader_validation", cause))?;
    Ok(())
}
