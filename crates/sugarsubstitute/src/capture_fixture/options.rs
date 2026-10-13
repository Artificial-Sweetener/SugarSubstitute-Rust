// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Parse one bounded exact internal role without filesystem or process effects.
use super::*;
use std::{
    ffi::{OsStr, OsString},
    path::{Component, Prefix},
};
use sugarsubstitute_contracts::lifecycle::{CoordinatorEpoch, NamespaceId};

/// Own bounded lexical role inputs without preparing native storage or processes.
pub(crate) struct Options {
    pub(crate) receipt: PathBuf,
    pub(super) coordinator: CoordinatorKey,
    pub(super) input: Option<CaptureStartInput>,
}
impl Options {
    /// Require exact root/coordinator/receipt options; release selection uses actual installed trust.
    pub(crate) fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self> {
        let mut arguments = arguments.into_iter();
        let root = PathBuf::from(value(&mut arguments, "--capture-root")?);
        let namespace = NamespaceId::new(hex::<32>(&value(&mut arguments, "--namespace")?)?);
        let epoch = CoordinatorEpoch::new(hex::<16>(&value(&mut arguments, "--epoch")?)?)?;
        let receipt = PathBuf::from(value(&mut arguments, "--receipt")?);
        if arguments.next().is_some() {
            return Err("Capture fixture rejects trailing arguments".into());
        }
        if receipt.capacity() > 32766 * 3
            || !receipt.is_absolute()
            || !matches!(receipt.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(b'D' | b'd') | Prefix::VerbatimDisk(b'D' | b'd')))
            || receipt.components().any(|c| matches!(c, Component::ParentDir))
            || receipt.as_os_str().to_string_lossy().contains('\0')
        {
            return Err("Capture receipt requires a bounded absolute D-drive path".into());
        }
        let input = CaptureStartInput::new(
            root,
            sugarsubstitute_supervision::InstalledReleaseOwner::new().capture_selection(),
        )?;
        Ok(Self { receipt, coordinator: CoordinatorKey { namespace, epoch }, input: Some(input) })
    }
}
/// Consume one ordered option pair without accepting mixed role syntax.
fn value(arguments: &mut impl Iterator<Item = OsString>, name: &str) -> Result<OsString> {
    if arguments.next().as_deref() != Some(OsStr::new(name)) {
        return Err("Capture fixture option order mismatch".into());
    }
    arguments.next().ok_or_else(|| "Capture fixture option value missing".into())
}
/// Decode exact lowercase identity/hash bytes; reject partial or alternate spellings.
pub(super) fn hex<const N: usize>(value: &OsStr) -> Result<[u8; N]> {
    let text = value.to_str().ok_or("Capture hex fact is not Unicode")?;
    if text.len() != N * 2 {
        return Err("Capture hex fact has wrong width".into());
    }
    let mut output = [0; N];
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        let digit = |v: u8| match v {
            b'0'..=b'9' => Ok(v - b'0'),
            b'a'..=b'f' => Ok(v - b'a' + 10),
            _ => Err("Capture hex fact must be lowercase ASCII"),
        };
        output[index] = (digit(pair[0])? << 4) | digit(pair[1])?;
    }
    Ok(output)
}
