// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe real passive copies and raw bounds while faking only the current-executable query.
use super::*;
use crate::{
    durable_storage::files,
    setup_storage::test_worker::{self, TestResult},
};
use std::path::PathBuf;
use sugarsubstitute_configuration::{descriptor::LaunchDescriptor, initial};
mod rejection;

struct Fixture {
    _temporary: tempfile::TempDir,
    image: PathBuf,
    paths: StartupPaths,
    controls: Controls,
}
impl Fixture {
    fn new(context: &JobContext, maximum: bool) -> io::Result<Self> {
        let temporary = tempfile::tempdir()?;
        let root = temporary.path();
        let image = root.join("SugarSubstitute.exe");
        std::fs::write(&image, b"actual retained image fixture; never executed")?;
        let paths = StartupPaths::new([
            root.join("startup.local.toml"),
            root.join("frontend"),
            root.join("METADATA"),
            root.join("SugarCubes"),
            root.join("main.py"),
            root.join("desktop-data/profile"),
        ])
        .map_err(io::Error::other)?;
        let mut startup = initial::external(
            &root.join("external Comfy café"),
            &root.join("python.exe"),
            &root.join("external output"),
            "en",
        )
        .map_err(io::Error::other)?
        .into_bytes();
        if maximum {
            startup.extend_from_slice(b"\n#");
            startup.resize(
                sugarsubstitute_configuration::startup::MAX_STARTUP_CONFIGURATION_BYTES - 1,
                b'x',
            );
            startup.push(b'\n');
        }
        std::fs::write(paths.configuration(), &startup)?;
        let duplicate =
            StartupPaths::new(paths.as_paths().map(Path::to_owned)).map_err(io::Error::other)?;
        let mut descriptor = LaunchDescriptor::encode(duplicate)?;
        if maximum {
            descriptor.resize(
                sugarsubstitute_configuration::startup::MAX_STARTUP_CONFIGURATION_BYTES,
                b' ',
            );
        }
        let descriptor_path = root.join("sugarsubstitute-launch.local.json");
        std::fs::write(&descriptor_path, descriptor)?;
        let mut controls = Controls::default();
        controls.resolve(
            sugarsubstitute_configuration::paths::ConfigurationSource::Installed(descriptor_path),
            context,
        )?;
        Ok(Self { _temporary: temporary, image, paths, controls })
    }
    fn snapshot(&self, context: &JobContext) -> io::Result<InvocationSnapshot> {
        let memory = context.memory().claim(RETAINED_BYTES).map_err(io::Error::other)?;
        let host = native::VerifiedHost::fixture(&self.image, context)?;
        Ok(InvocationSnapshot {
            host,
            stage: None,
            final_files: None,
            reference: None,
            _memory: memory,
        })
    }
}

#[test]
fn each_invocation_keeps_current_exact_raw_originals_and_distinct_copy_native_ids() -> TestResult {
    test_worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let mut fixture = Fixture::new(context, true)?;
        let raw = fixture.controls.startup.as_ref().ok_or("Startup original absent")?.bytes.clone();
        let descriptor =
            fixture.controls.descriptor.as_ref().ok_or("Descriptor original absent")?.bytes.clone();
        let original_identity = files::native_identity(
            &fixture.controls.startup.as_ref().ok_or("Startup original absent")?.file,
        )?;
        let mut first = fixture.snapshot(context)?;
        first.publish(&fixture.controls, &fixture.paths, context)?;
        let first_reference = first.reference().ok_or("Published reference absent")?;
        let pinned = first.final_files.as_ref().ok_or("Final passive custody absent")?;
        let manifest = pinned.manifest.as_ref().ok_or("Final manifest absent")?;
        assert_eq!(pinned.startup, raw);
        assert_eq!(pinned.descriptor.as_deref(), Some(descriptor.as_slice()));
        assert_eq!(manifest.startup.original_identity, original_identity);
        assert_ne!(manifest.startup.copy_identity, original_identity);
        assert!(std::fs::write(fixture.paths.configuration(), b"before adoption").is_err());
        let path = fixture.paths.configuration().to_owned();
        fixture.controls = Controls::default();
        std::fs::write(&path, &raw)?;
        assert_eq!(first.final_files.as_ref().ok_or("Passive custody lost")?.startup, raw);
        let mut changed = b"# a later valid current invocation\r\n".to_vec();
        changed.extend_from_slice(&raw[..raw.len() - 128]);
        changed.push(b'\n');
        std::fs::write(&path, &changed)?;
        fixture.controls.resolve(
            sugarsubstitute_configuration::paths::ConfigurationSource::Explicit(StartupPaths::new(
                fixture.paths.as_paths().map(Path::to_owned),
            )?),
            context,
        )?;
        let mut second = fixture.snapshot(context)?;
        second.publish(&fixture.controls, &fixture.paths, context)?;
        assert_ne!(second.reference(), Some(first_reference));
        assert_eq!(
            second.final_files.as_ref().ok_or("Second passive custody absent")?.startup,
            changed
        );
        assert_eq!(first.final_files.as_ref().ok_or("First passive custody absent")?.startup, raw);
        Ok(())
    })
}

#[test]
fn native_producer_mismatch_precedes_storage_creation_and_snapshot_documents_stay_pinned()
-> TestResult {
    test_worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let fixture = Fixture::new(context, false)?;
        let root = fixture.image.parent().ok_or("Fixture image parent absent")?;
        assert!(InvocationSnapshot::begin(root, context).is_err());
        assert!(!root.join("user").exists());
        let mut snapshot = fixture.snapshot(context)?;
        snapshot.publish(&fixture.controls, &fixture.paths, context)?;
        let reference = snapshot.reference().ok_or("Published reference absent")?;
        let parent = snapshot.host.originals_directory(false, context)?;
        let directory = parent.child(&crate::durable_storage::protected::record_file::hex(
            reference.manifest_sha256(),
        ))?;
        let original = std::fs::read(directory.path().join("startup.toml"))?;
        assert!(std::fs::write(directory.path().join("startup.toml"), b"changed copy").is_err());
        assert_eq!(std::fs::read(directory.path().join("startup.toml"))?, original);
        assert!(std::fs::rename(directory.path(), parent.path().join("foreign-copy")).is_err());
        Ok(())
    })
}
