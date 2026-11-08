// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish complete current-invocation raw copies before exposing a digest reference.
use super::{
    manifest::{self, Manifest, Original, SourceKind},
    *,
};
use crate::durable_storage::{
    files, protected::directories::Mode, protection, record_publication, staging,
};
use sha2::{Digest, Sha256};

pub(super) fn publish(
    snapshot: &mut InvocationSnapshot,
    controls: &Controls,
    paths: &StartupPaths,
    context: &JobContext,
) -> io::Result<()> {
    if controls.ready.is_some() {
        return Err(invalid("Source invocation cannot infer installed readiness ownership"));
    }
    let original =
        controls.startup.as_ref().ok_or_else(|| invalid("Captured startup original absent"))?;
    let parent = snapshot.host.originals_directory(true, context)?;
    let stage_path = staging::unique_path(&parent, ".entry-originals")?;
    protection::with(context, |owner| {
        owner.directory(&stage_path, Mode::CreateNew, context)?;
        Ok(())
    })?;
    snapshot.stage = Some(Directory::open(&stage_path, false)?.resolved()?);
    let stage =
        snapshot.stage.as_ref().ok_or_else(|| invalid("Invocation stage custody absent"))?;
    let startup = copy(stage, "startup.toml", original, context)?;
    let descriptor = controls
        .descriptor
        .as_ref()
        .map(|value| copy(stage, "descriptor.json", value, context))
        .transpose()?;
    let mut captured_paths: [Vec<u16>; 6] = std::array::from_fn(|_| Vec::new());
    for (target, path) in captured_paths.iter_mut().zip(paths.as_paths()) {
        *target = manifest::units(path)?;
    }
    let manifest = Manifest {
        schema_version: 1,
        capture: staging::identity()?,
        producer: snapshot.host.producer.clone(),
        directory_identity: stage.native_identity()?,
        source_kind: if descriptor.is_some() {
            SourceKind::Descriptor
        } else {
            SourceKind::Explicit
        },
        paths: captured_paths,
        startup,
        descriptor,
    };
    let encoded = serde_json::to_vec(&manifest).map_err(io::Error::other)?;
    let verified = Manifest::decode(&encoded)?;
    verified.intent(
        &original.bytes,
        controls.descriptor.as_ref().map(|value| value.bytes.as_slice()),
    )?;
    record_publication::publish(
        stage,
        manifest::MANIFEST,
        &encoded,
        manifest::MAX_MANIFEST_BYTES,
        context,
    )?;
    let reference =
        EntryOriginalsReference::new(Sha256::digest(&encoded).into()).map_err(io::Error::other)?;
    let identity = stage.identity()?;
    let source = stage.path().to_owned();
    // Ordinary pins deny delete sharing. Close the entire stage chain before the existing
    // handle-based no-replacement publisher reopens, verifies and renames that native object.
    snapshot.stage = None;
    let name = crate::durable_storage::protected::record_file::hex(reference.manifest_sha256());
    protection::with(context, |owner| {
        owner.publish(
            &source,
            &parent.path().join(name),
            identity,
            protection::Publication::Directory,
            context,
        )
    })?;
    snapshot.final_files = Some(reading::PinnedOriginals::new(&parent, reference)?);
    snapshot.final_files.as_mut().ok_or_else(|| invalid("Final invocation custody absent"))?.load(
        reference,
        &snapshot.host.producer,
        context,
    )?;
    check(context)?;
    snapshot.reference = Some(reference);
    Ok(())
}

fn copy(
    parent: &Directory,
    name: &str,
    original: &super::super::controls::ControlFile,
    context: &JobContext,
) -> io::Result<Original> {
    record_publication::publish(
        parent,
        name,
        &original.bytes,
        sugarsubstitute_configuration::startup::MAX_STARTUP_CONFIGURATION_BYTES,
        context,
    )?;
    protection::with(context, |owner| {
        let mut file = parent.read(name)?;
        let result = (|| {
            files::single_link(&file)?;
            owner.verify_file(&file)?;
            let bytes = files::read_admitted(
                &mut file,
                sugarsubstitute_configuration::startup::MAX_STARTUP_CONFIGURATION_BYTES,
                context,
            )?;
            if bytes != original.bytes {
                return Err(invalid(
                    "Invocation passive copy differs from captured original bytes",
                ));
            }
            Ok(Original {
                source: manifest::units(&original.path)?,
                original_identity: files::native_identity(&original.file)?,
                copy_identity: files::native_identity(&file)?,
                bytes: bytes.len() as u64,
                sha256: Sha256::digest(&bytes).into(),
            })
        })();
        let closed = owner.close_file(file);
        match (result, closed) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(cause), Ok(())) | (Ok(_), Err(cause)) => Err(cause),
            (Err(operation), Err(cause)) => {
                Err(io::Error::other(protection::DomainFailure::Native {
                    cause,
                    operation: Some(operation),
                }))
            }
        }
    })
}
