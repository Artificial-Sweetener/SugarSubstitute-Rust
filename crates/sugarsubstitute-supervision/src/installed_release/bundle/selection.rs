// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authenticate original manifest ingress before any label-directed native open.
use super::super::{
    anchor,
    image::PeKind,
    manifest::{Manifest, Pair},
    names::CapturePathRole,
};
use super::*;
use std::ffi::OsStr;
struct Record {
    pe: [u8; 128],
    pe_len: usize,
    pdb: [u8; 128],
    pdb_len: usize,
    facts: Pair<'static>,
}
impl Record {
    fn copy(pair: &Pair<'_>) -> Result<Self, ClaimError> {
        let mut pe = [0; 128];
        let mut pdb = [0; 128];
        let destination = pe.get_mut(..pair.pe_label.len()).ok_or_else(missing)?;
        destination.copy_from_slice(pair.pe_label.as_bytes());
        let destination = pdb.get_mut(..pair.pdb_label.len()).ok_or_else(missing)?;
        destination.copy_from_slice(pair.pdb_label.as_bytes());
        let facts = Pair { pe_label: "", pdb_label: "", ..*pair };
        Ok(Self { pe, pe_len: pair.pe_label.len(), pdb, pdb_len: pair.pdb_label.len(), facts })
    }
    fn pe(&self) -> Result<&str, ClaimError> {
        std::str::from_utf8(&self.pe[..self.pe_len]).map_err(|_| missing())
    }
    fn pdb(&self) -> Result<&str, ClaimError> {
        std::str::from_utf8(&self.pdb[..self.pdb_len]).map_err(|_| missing())
    }
}
#[derive(Clone, Copy)]
pub(super) enum BundlePurpose {
    Capture,
    Symbol,
}
pub(super) fn select(
    fields: &mut Fields,
    origin: &AllocationOrigin,
    purpose: BundlePurpose,
) -> Result<(), ClaimError> {
    let anchor = anchor::selected().ok_or_else(|| {
        fields.causes[0].as_mut().map_or_else(missing, |causes| {
            causes.error(
                "missing_trusted_release",
                crate::claim_error::BoundedNativeCause::RawIo(std::io::Error::from(
                    std::io::ErrorKind::PermissionDenied,
                )),
                false,
            )
        })
    })?;
    {
        let causes = fields.causes[0].as_mut().ok_or_else(missing)?;
        let mut phase = files::phase(origin, &mut fields.scratch, causes);
        fields.current = Some(super::super::current_image::read(origin, &mut phase)?);
    }
    files::account(fields, origin)?;
    files::ancestors(fields, origin, false)?;
    files::open(fields, origin, 0, CapturePathRole::CurrentImage, None, u64::MAX)?;
    files::open(
        fields,
        origin,
        1,
        CapturePathRole::Manifest,
        Some("sugarsubstitute-release-v1.manifest"),
        65_536,
    )?;
    files::open(
        fields,
        origin,
        2,
        CapturePathRole::Signature,
        Some("sugarsubstitute-release-v1.sig"),
        384,
    )?;
    files::ingress(fields, origin, 1, false)?;
    files::ingress(fields, origin, 2, true)?;
    let context = origin.original().original_context().map_err(|_| missing())?.clone();
    fields.crypto.as_mut().ok_or_else(missing)?.verify(
        &fields.manifest,
        &fields.signature,
        &context,
    )?;
    // Parsing is pure; the original CNG success precedes every label-directed open.
    let (host, capture) = {
        let manifest = Manifest::parse(&fields.manifest).map_err(|cause| {
            fields.causes[3].as_mut().map_or_else(missing, |causes| {
                causes.selected_error(
                    "selected_manifest_format",
                    super::super::CaptureBoundedCause::Manifest(cause),
                    false,
                )
            })
        })?;
        if &manifest.key_id != anchor.key_id {
            return Err(missing());
        }
        #[cfg(feature = "local-k1-symbol-trust")]
        manifest.require_compiled_build().map_err(|cause| {
            fields.causes[3].as_mut().map_or_else(missing, |causes| {
                causes.selected_error(
                    "selected_manifest_build",
                    super::super::CaptureBoundedCause::Manifest(cause),
                    false,
                )
            })
        })?;
        let host = manifest.role_pair().ok_or_else(missing)?;
        let leaf = fields
            .current
            .as_ref()
            .ok_or_else(missing)?
            .path()
            .map_err(ClaimError::capture_allocation)?
            .file_name()
            .ok_or_else(missing)?;
        if leaf != OsStr::new(host.pe_label) {
            return Err(missing());
        }
        let capture = match purpose {
            BundlePurpose::Capture => {
                let (client, handler) = manifest.capture_pairs().map_err(|cause| {
                    fields.causes[3].as_mut().map_or_else(missing, |causes| {
                        causes.selected_error(
                            "selected_capture_role",
                            super::super::CaptureBoundedCause::Manifest(cause),
                            false,
                        )
                    })
                })?;
                Some((
                    Record::copy(client)?,
                    Record::copy(handler)?,
                    CaptureRegistrationFacts::new(
                        manifest.build,
                        client.pe_sha256,
                        handler.pe_sha256,
                    )?,
                ))
            }
            BundlePurpose::Symbol => None,
        };
        let symbol = SymbolRegistrationFacts::new(manifest.build, host)?;
        (Record::copy(host)?, (capture, symbol))
    };
    files::open(
        fields,
        origin,
        3,
        CapturePathRole::CurrentPdb,
        Some(host.pdb()?),
        host.facts.pdb_length,
    )?;
    files::verify_pair(fields, origin, 0, 3, &host.facts, PeKind::Executable)?;
    if let Some((client, handler, _)) = &capture.0 {
        files::open(
            fields,
            origin,
            4,
            CapturePathRole::ClientImage,
            Some(client.pe()?),
            client.facts.pe_length,
        )?;
        files::open(
            fields,
            origin,
            5,
            CapturePathRole::ClientPdb,
            Some(client.pdb()?),
            client.facts.pdb_length,
        )?;
        files::verify_pair(fields, origin, 4, 5, &client.facts, PeKind::Library)?;
        files::open(
            fields,
            origin,
            6,
            CapturePathRole::HandlerImage,
            Some(handler.pe()?),
            handler.facts.pe_length,
        )?;
        files::open(
            fields,
            origin,
            7,
            CapturePathRole::HandlerPdb,
            Some(handler.pdb()?),
            handler.facts.pdb_length,
        )?;
        files::verify_pair(fields, origin, 6, 7, &handler.facts, PeKind::Executable)?;
    }
    files::ancestors(fields, origin, true)?;
    origin.original().check_effect().map_err(|cause| {
        fields.causes[3]
            .as_mut()
            .map_or_else(missing, |causes| causes.selected_control(cause, false))
    })?;
    match purpose {
        BundlePurpose::Capture => fields.facts = capture.0.map(|(_, _, facts)| facts),
        BundlePurpose::Symbol => fields.symbol_facts = Some(capture.1),
    }
    Ok(())
}
