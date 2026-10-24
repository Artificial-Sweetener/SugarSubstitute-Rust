// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Superseded unfinished first installations remain retryable without discarding earlier files.
use super::*;
mod guards;
mod interruption;

/// Sign distinct payload inventories with the same repository and increasing metadata versions.
fn version(path: &Path, release: u32, context: &JobContext) -> Result<SignedInstallation, Failure> {
    import_version(InstallationSession::create(choices(path), context)?, path, release, context)
}

/// Use the caller's fresh or maintenance session without changing its saved configuration.
fn import_version(
    session: InstallationSession,
    path: &Path,
    release: u32,
    context: &JobContext,
) -> Result<SignedInstallation, Failure> {
    let (manifest, compressed) = test_package::distribution()?;
    let mut manifest: serde_json::Value = serde_json::from_slice(&manifest)?;
    manifest["release_id"] = format!("initial-release-{release}").into();
    let archive = test_package::seal(&serde_json::to_vec(&manifest)?, &compressed);
    let signed = SignedPackage::new(&archive, release)?;
    let source = path.join(format!("setup-{release}.exe"));
    if !source.exists() {
        std::fs::write(&source, [b"inert prefix".as_slice(), &archive].concat())?;
    }
    Ok(session.import_installer(
        InstallerSource {
            repository: [92; 32],
            trusted_root: &signed.root,
            roots: &[],
            metadata: signed.metadata.metadata(),
            descriptor: &signed.descriptor,
            executable: &source,
        },
        signed.request,
        BundleSource::Extract(&path.join("workspace")),
        || NOW,
        context,
    )?)
}

#[test]
fn unfinished_first_installation_can_resume_after_repeated_supersession() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path();
        let runtime = runtime(path)?;
        let mut preserved = Vec::new();
        for release in 1..=3 {
            let prepared = version(path, release, context)?.prepare(|| NOW, context)?;
            assert!(prepared.maintenance_runtime().is_none());
            let record = SelectionStore::open(&path.join("native"), context)?
                .load(context)?
                .ok_or("Selection absent")?;
            assert!(record.accepted().is_none());
            let generation = record.candidate().ok_or("Candidate absent")?.generation();
            let note = path.join("native/releases").join(generation.id.as_str()).join("user-note");
            std::fs::write(&note, b"preserve unfinished generation")?;
            preserved.push(note);
            drop(prepared);
            assert!(inspect_installation(&path.join("native"), context)?.is_none());
        }
        let candidate = version(path, 3, context)?.prepare(|| NOW, context)?.complete(
            &runtime,
            || NOW,
            context,
        )?;
        for note in preserved {
            assert_eq!(std::fs::read(note)?, b"preserve unfinished generation");
        }
        assert!(candidate.directory().join("sugarsubstitute.exe").is_file());
        assert_eq!(std::fs::read(runtime.python)?, b"inert, never executed");
        Ok(())
    })
}
