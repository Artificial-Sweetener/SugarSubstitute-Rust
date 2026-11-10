// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound extra argv using the installed ComfyUI CLI's complete names and arities.
//! These are input-shape metadata, not a second implementation of option values.
use super::ManagedLaunchError;
#[derive(Clone, Copy)]
enum Arity {
    Flag,
    One,
    Optional,
    Many,
    Some,
}
const OPTIONS: &[(&str, Arity)] = &[
    ("--listen", Arity::Optional),
    ("--port", Arity::One),
    ("--tls-keyfile", Arity::One),
    ("--tls-certfile", Arity::One),
    ("--enable-cors-header", Arity::Optional),
    ("--max-upload-size", Arity::One),
    ("--base-directory", Arity::One),
    ("--extra-model-paths-config", Arity::Some),
    ("--output-directory", Arity::One),
    ("--temp-directory", Arity::One),
    ("--input-directory", Arity::One),
    ("--auto-launch", Arity::Flag),
    ("--disable-auto-launch", Arity::Flag),
    ("--cuda-device", Arity::One),
    ("--default-device", Arity::One),
    ("--cuda-malloc", Arity::Flag),
    ("--disable-cuda-malloc", Arity::Flag),
    ("--force-fp32", Arity::Flag),
    ("--force-fp16", Arity::Flag),
    ("--fp32-unet", Arity::Flag),
    ("--fp64-unet", Arity::Flag),
    ("--bf16-unet", Arity::Flag),
    ("--fp16-unet", Arity::Flag),
    ("--fp8_e4m3fn-unet", Arity::Flag),
    ("--fp8_e5m2-unet", Arity::Flag),
    ("--fp8_e8m0fnu-unet", Arity::Flag),
    ("--fp16-vae", Arity::Flag),
    ("--fp32-vae", Arity::Flag),
    ("--bf16-vae", Arity::Flag),
    ("--cpu-vae", Arity::Flag),
    ("--fp8_e4m3fn-text-enc", Arity::Flag),
    ("--fp8_e5m2-text-enc", Arity::Flag),
    ("--fp16-text-enc", Arity::Flag),
    ("--fp32-text-enc", Arity::Flag),
    ("--bf16-text-enc", Arity::Flag),
    ("--fp16-intermediates", Arity::Flag),
    ("--force-channels-last", Arity::Flag),
    ("--directml", Arity::Optional),
    ("--oneapi-device-selector", Arity::One),
    ("--supports-fp8-compute", Arity::Flag),
    ("--enable-triton-backend", Arity::Flag),
    ("--disable-triton-backend", Arity::Flag),
    ("--preview-method", Arity::One),
    ("--preview-size", Arity::One),
    ("--cache-ram", Arity::Many),
    ("--cache-classic", Arity::Flag),
    ("--cache-lru", Arity::One),
    ("--cache-none", Arity::Flag),
    ("--high-ram", Arity::Flag),
    ("--use-split-cross-attention", Arity::Flag),
    ("--use-quad-cross-attention", Arity::Flag),
    ("--use-pytorch-cross-attention", Arity::Flag),
    ("--use-sage-attention", Arity::Flag),
    ("--use-flash-attention", Arity::Flag),
    ("--disable-xformers", Arity::Flag),
    ("--force-upcast-attention", Arity::Flag),
    ("--dont-upcast-attention", Arity::Flag),
    ("--enable-manager", Arity::Flag),
    ("--disable-manager-ui", Arity::Flag),
    ("--enable-manager-legacy-ui", Arity::Flag),
    ("--gpu-only", Arity::Flag),
    ("--highvram", Arity::Flag),
    ("--lowvram", Arity::Flag),
    ("--novram", Arity::Flag),
    ("--cpu", Arity::Flag),
    ("--reserve-vram", Arity::One),
    ("--vram-headroom", Arity::One),
    ("--async-offload", Arity::Optional),
    ("--disable-async-offload", Arity::Flag),
    ("--disable-dynamic-vram", Arity::Flag),
    ("--enable-dynamic-vram", Arity::Flag),
    ("--fast-disk", Arity::Flag),
    ("--force-non-blocking", Arity::Flag),
    ("--default-hashing-function", Arity::One),
    ("--disable-smart-memory", Arity::Flag),
    ("--deterministic", Arity::Flag),
    ("--fast", Arity::Many),
    ("--debug-hang", Arity::Flag),
    ("--disable-pinned-memory", Arity::Flag),
    ("--mmap-torch-files", Arity::Flag),
    ("--disable-mmap", Arity::Flag),
    ("--dont-print-server", Arity::Flag),
    ("--quick-test-for-ci", Arity::Flag),
    ("--windows-standalone-build", Arity::Flag),
    ("--disable-metadata", Arity::Flag),
    ("--disable-all-custom-nodes", Arity::Flag),
    ("--whitelist-custom-nodes", Arity::Some),
    ("--disable-api-nodes", Arity::Flag),
    ("--multi-user", Arity::Flag),
    ("--verbose", Arity::Many),
    ("--log-stdout", Arity::Flag),
    ("--front-end-version", Arity::One),
    ("--front-end-root", Arity::One),
    ("--user-directory", Arity::One),
    ("--models-directory", Arity::One),
    ("--enable-compress-response-body", Arity::Flag),
    ("--comfy-api-base", Arity::One),
    ("--database-url", Arity::One),
    ("--enable-assets", Arity::Flag),
    ("--enable-asset-hashing", Arity::Flag),
    ("--feature-flag", Arity::One),
    ("--list-feature-flags", Arity::Flag),
];
pub(super) const PROTECTED: &[&str] = &[
    "--listen",
    "--port",
    "--tls-keyfile",
    "--tls-certfile",
    "--auto-launch",
    "--disable-auto-launch",
    "--windows-standalone-build",
    "--front-end-version",
    "--front-end-root",
    "--disable-all-custom-nodes",
    "--whitelist-custom-nodes",
    "--quick-test-for-ci",
    "--list-feature-flags",
    "--help",
];
pub(super) fn validate(args: &[String]) -> Result<(), ManagedLaunchError> {
    visit(args, |_, _| {})
}
/// Lend complete accepted options to storage inspection through this same arity grammar.
pub(super) fn visit<'a>(
    args: &'a [String],
    mut accepted: impl FnMut(&str, Option<&'a str>),
) -> Result<(), ManagedLaunchError> {
    let mut index = 0;
    while index < args.len() {
        let argument = &args[index];
        let (name, inline) =
            argument.split_once('=').map_or((argument.as_str(), None), |(a, b)| (a, Some(b)));
        if name.starts_with("--") && name.len() > 2 && PROTECTED.iter().any(|p| p.starts_with(name))
        {
            return Err(ManagedLaunchError::ProtectedOption);
        }
        let arity = OPTIONS
            .iter()
            .find_map(|(option, arity)| (*option == name).then_some(*arity))
            .ok_or(if argument.starts_with('-') {
                ManagedLaunchError::UnknownOption
            } else {
                ManagedLaunchError::ArgumentShape
            })?;
        index += 1;
        let value = inline.or_else(|| args.get(index).map(String::as_str));
        let mut count = usize::from(inline.is_some());
        if inline.is_none() && !matches!(arity, Arity::Flag) {
            while index < args.len() && !is_option(&args[index]) {
                count += 1;
                index += 1;
                if matches!(arity, Arity::One | Arity::Optional) {
                    break;
                }
            }
        }
        let valid = match arity {
            Arity::Flag => count == 0,
            Arity::One => count == 1,
            Arity::Optional => count <= 1,
            Arity::Many => true,
            Arity::Some => count >= 1,
        };
        if !valid {
            return Err(ManagedLaunchError::ArgumentShape);
        }
        accepted(name, value);
    }
    Ok(())
}
fn is_option(text: &str) -> bool {
    // Argparse treats a syntactically negative number as a value; other dash
    // prefixes are option tokens even when the preceding option takes a value.
    text.starts_with('-') && !negative_number(text)
}
fn negative_number(text: &str) -> bool {
    let Some(value) = text.strip_prefix('-') else { return false };
    let mut dot = false;
    let mut digit = false;
    for byte in value.bytes() {
        if byte.is_ascii_digit() {
            digit = true;
        } else if byte == b'.' && !dot {
            dot = true;
        } else {
            return false;
        }
    }
    digit && !value.ends_with('.')
}
