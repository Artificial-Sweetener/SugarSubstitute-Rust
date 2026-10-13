// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind every fixed-record output to immutable bytes and original admission.
use super::*;
use crate::claims::entry::{FreshCoordinatorPublication, FreshPublicationBinding};
use std::path::PathBuf;
use sugarsubstitute_contracts::execution::JobContext;
/// Borrow actual initialization proof or retain ordinary predecessor policy.
pub(crate) enum PublicationAuthority<'a> {
    /// Require owner predecessor policy under actual locked exclusion.
    Ordinary,
    /// Require the actual sealed election and claim lifetime as well.
    FreshCoordinator(&'a FreshCoordinatorPublication<'a>),
}
/// Retain admission before reads can leave a failed-close output without a stage.
pub(crate) struct PublicationBinding<const N: usize> {
    fresh: Option<FreshPublicationBinding>,
    encoded: Option<[u8; N]>,
    pub(super) context: context::PublicationContext,
}
impl PublicationAuthority<'_> {
    fn bind<const N: usize>(
        &self,
        encoded: &[u8; N],
        current: &JobContext,
    ) -> Result<PublicationBinding<N>, ClaimError> {
        let context = context::PublicationContext::new(current)?;
        let fresh = match self {
            Self::Ordinary => None,
            Self::FreshCoordinator(proof) => Some(proof.binding()?),
        };
        if fresh.as_ref().is_some_and(|proof| proof.execution_owner() != current.owner_id()) {
            return Err(ClaimError::invalid("foreign_fresh_publication_context"));
        }
        Ok(PublicationBinding { context, encoded: Some(*encoded), fresh })
    }
    fn validate_retained<const N: usize>(&self, file: &HeldFile<N>) -> Result<(), ClaimError> {
        match self {
            Self::Ordinary if file.is_locked() => Ok(()),
            Self::FreshCoordinator(proof) => proof.validate_retained(file),
            _ => Err(ClaimError::invalid("publication_without_exclusion")),
        }
    }
    fn validate<const N: usize>(
        &self,
        file: &HeldFile<N>,
        old: &RecordRead<N>,
    ) -> Result<(), ClaimError> {
        self.validate_retained(file)?;
        match self {
            Self::Ordinary => match old {
                RecordRead::Empty => Err(ClaimError::invalid("empty_claim_record")),
                RecordRead::Missing if !file.created => {
                    Err(ClaimError::invalid("missing_claim_record"))
                }
                _ => Ok(()),
            },
            Self::FreshCoordinator(proof) => proof.validate(file, old),
        }
    }
}
impl<const N: usize> PublicationBinding<N> {
    /// Retain an observation's ports while a failed source close remains owned.
    pub(super) fn read(current: &JobContext) -> Result<Self, ClaimError> {
        Ok(Self { fresh: None, encoded: None, context: context::PublicationContext::new(current)? })
    }
    /// Retain cleanup ports before a retirement read can leave a partial output.
    pub(super) fn cleanup(
        expected: Option<&[u8; N]>,
        current: &JobContext,
    ) -> Result<Self, ClaimError> {
        let mut context = context::PublicationContext::new(current)?;
        context.validate_cleanup(current)?;
        Ok(Self { fresh: None, encoded: expected.copied(), context })
    }
    fn validate(
        &mut self,
        file: &HeldFile<N>,
        authority: &PublicationAuthority<'_>,
        encoded: &[u8; N],
        current: &JobContext,
    ) -> Result<(), ClaimError> {
        if self.encoded.as_ref() != Some(encoded) {
            return Err(ClaimError::invalid("changed_publication_generation"));
        }
        match (&self.fresh, authority) {
            (None, PublicationAuthority::Ordinary) if file.is_locked() => (),
            (Some(original), PublicationAuthority::FreshCoordinator(next)) => {
                original.validate(file, next)?
            }
            _ => return Err(ClaimError::invalid("changed_publication_authority")),
        }
        self.context.validate(current)
    }
}
/// Own actual staging inode and activation before any post-effect context check.
pub(crate) struct Stage<const N: usize> {
    pub(crate) handle: Handle,
    pub(super) path: PathBuf,
    pub(super) identity: Option<[u8; 24]>,
    pub(super) activated: Option<[u8; N]>,
    pub(super) encoded: [u8; N],
}
/// Publish immutable bytes under the actual admitted context and record authority.
/// # Errors
/// Context, predecessor and IO failures retain adopted outputs until checked
/// retirement. A failed stage cannot become another generation on retry.
pub(crate) fn publish<const N: usize>(
    file: &HeldFile<N>,
    encoded: &[u8; N],
    authority: PublicationAuthority<'_>,
    context: &JobContext,
    validate_predecessor: impl FnOnce(&RecordRead<N>) -> Result<(), ClaimError>,
) -> Result<(), ClaimError> {
    publish_with_boundary(
        file,
        encoded,
        authority,
        context,
        validate_predecessor,
        &mut native::NativeRecordBoundary,
    )
}
pub(crate) fn publish_with_boundary<const N: usize>(
    file: &HeldFile<N>,
    encoded: &[u8; N],
    authority: PublicationAuthority<'_>,
    context: &JobContext,
    validate_predecessor: impl FnOnce(&RecordRead<N>) -> Result<(), ClaimError>,
    boundary: &mut dyn RecordBoundary<N>,
) -> Result<(), ClaimError> {
    if !file.handle.budget().is_ordinary() {
        return Err(file.handle.budget().rejection("fixed_record_ordinary_origin"));
    }
    if !file.is_locked() || !(16..=1024).contains(&N) {
        return Err(ClaimError::invalid("publication_without_exclusion"));
    }
    let mut pending = lock(&file.publication);
    if pending.is_none() {
        *pending = Some(authority.bind(encoded, context)?);
    }
    let binding =
        pending.as_mut().ok_or_else(|| ClaimError::invalid("missing_publication_binding"))?;
    macro_rules! step {
        ($effect:expr) => {{
            binding.validate(file, &authority, encoded, context)?;
            let result = $effect;
            ClaimError::finish(result, binding.validate(file, &authority, encoded, context))?
        }};
    }
    step!(boundary.verify_exclusion(file));
    let mut retained = lock(&file.stage);
    if let Some(stage) = retained.as_mut() {
        if stage.encoded != *encoded {
            return Err(ClaimError::invalid("changed_publication_generation"));
        }
        if stage.activated.is_some() {
            let mut activated = stage.activated;
            finish_activation(&mut activated, encoded, || {
                settle_activation(file, stage, boundary, || {
                    binding.validate(file, &authority, encoded, context)
                })
            })?;
            stage.activated = activated;
            *retained = None;
            *pending = None;
            return Ok(());
        }
    }
    let old = step!(
        boundary
            .read_shape(file, None, &mut || binding.validate(file, &authority, encoded, context))
    );
    if let Err(cause) = authority.validate(file, &old).and_then(|()| validate_predecessor(&old)) {
        if retained.is_none() {
            *pending = None;
        }
        return Err(cause);
    }
    binding.validate(file, &authority, encoded, context)?;
    if retained.is_none() {
        *retained = Some(Stage {
            handle: Handle::new(
                windows::Win32::Foundation::HANDLE::default(),
                file.handle.budget(),
            ),
            path: PathBuf::new(),
            identity: None,
            activated: None,
            encoded: *encoded,
        });
        let stage = retained.as_mut().ok_or_else(|| ClaimError::invalid("missing_stage"))?;
        step!(boundary.create_stage_into(file, stage));
    }
    let stage = retained.as_mut().ok_or_else(|| ClaimError::invalid("missing_stage"))?;
    if !boundary.stage_is_open(stage) {
        step!(boundary.reopen_stage_into(file, stage));
    }
    step!(boundary.verify_stage(file, stage));
    step!(
        boundary.write_stage(stage, encoded, &mut || binding
            .validate(file, &authority, encoded, context))
    );
    step!(boundary.activate_stage(file, stage));
    let mut activated = stage.activated;
    finish_activation(&mut activated, encoded, || {
        settle_activation(file, stage, boundary, || {
            binding.validate(file, &authority, encoded, context)
        })
    })?;
    stage.activated = activated;
    *retained = None;
    *pending = None;
    Ok(())
}
pub(super) fn settle_activation<const N: usize>(
    file: &HeldFile<N>,
    stage: &mut Stage<N>,
    boundary: &mut dyn RecordBoundary<N>,
    mut check: impl FnMut() -> Result<(), ClaimError>,
) -> Result<(), ClaimError> {
    let actual = stage.activated.ok_or_else(|| ClaimError::invalid("missing_stage_activation"))?;
    macro_rules! step {
        ($effect:expr) => {{
            check()?;
            let result = $effect;
            ClaimError::finish(result, check())?
        }};
    }
    if boundary.stage_is_open(stage) {
        step!(boundary.verify_stage(file, stage));
        step!(boundary.read_activated(stage, &actual, &mut check));
        step!(boundary.flush_stage(stage));
        step!(boundary.close_stage(stage));
    }
    let identity =
        stage.identity.ok_or_else(|| ClaimError::invalid("missing_activated_identity"))?;
    if !matches!(step!(boundary.read_shape(file,Some(identity), &mut check)),RecordRead::Present(bytes) if bytes==actual)
    {
        return Err(ClaimError::invalid("changed_activated_record"));
    }
    Ok(())
}
