// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual copied-file IDs, canonical matching and immutable receipt rejection at the native port.
use super::*;
use crate::{
    durable_storage::protected::directories::Mode,
    setup_storage::test_worker::{self, TestResult},
};
use std::path::PathBuf;
use sugarsubstitute_contracts::installation::InstallationId;

struct Fixture {
    _temporary: tempfile::TempDir,
    root: Directory,
    original: Directory,
    target: PathBuf,
    installation: InstallationId,
    namespace: NamespaceId,
    name: String,
}
impl Fixture {
    fn new(context: &JobContext) -> io::Result<Self> {
        let temporary = test_worker::installation(0xd4, context).map_err(io::Error::other)?;
        let root = Directory::open(temporary.path(), false)?;
        let namespace = NamespaceId::new([7; 32]);
        let leaf = record_file::DiagnosticLeaf::object(DiagnosticObject::StoreState, namespace)
            .map_err(io::Error::other)?;
        let name = leaf.text().map_err(io::Error::other)?.to_owned();
        let original = protection::with(context, |owner| {
            let path = root.path().join("old-diagnostic-namespace");
            owner.directory(&path, Mode::CreateNew, context)?;
            Directory::open(&path, false)
        })?;
        // The native source does not interpret the ledger codec. Its sole concern is the actual
        // protected copied generation; the crash owner independently validates these bytes.
        std::fs::write(
            original.path().join(&name),
            b"original bytes await the sole incident codec",
        )?;
        std::fs::write(original.path().join(record_file::DIAGNOSTIC_WRITER_LEAF), b"")?;
        let parent = root.path().join("user/migration-originals/diagnostic-namespaces");
        crate::durable_storage::user_paths::create_parents(&root, &parent, context)?;
        let target = parent.join(record_file::hex(namespace.bytes()));
        let installation =
            InstallationId::new(format!("{:032x}", 0xd4)).map_err(io::Error::other)?;
        imports::copy_tree(
            &root,
            imports::ImportAssociation::Installed(&installation),
            &original,
            &target,
            imports::RootEntries::Except(&[]),
            context,
        )?;
        Ok(Self { _temporary: temporary, root, original, target, installation, namespace, name })
    }
    fn open(&self, context: &JobContext) -> io::Result<PreservedDiagnosticSource> {
        PreservedDiagnosticSource::open_at(
            &self.root,
            imports::ImportAssociation::Installed(&self.installation),
            self.namespace,
            context,
        )
    }
}

#[test]
fn native_passive_source_reads_new_file_ids_and_keeps_canonical_copy_pins_without_old_writer()
-> TestResult {
    test_worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let fixture = Fixture::new(context)?;
        let original = fixture.original.read(&fixture.name)?;
        let copied = Directory::open(&fixture.target, false)?.read(&fixture.name)?;
        assert_ne!(files::native_identity(&original)?, files::native_identity(&copied)?);
        drop(copied);
        let mut source = fixture.open(context)?;
        assert_eq!(source.entries().len(), 1);
        assert!(source.matches(0, DiagnosticObject::StoreState)?);
        let mut buffer = [0; 64];
        let count = source.read_chunk(0, 0, &mut buffer)?;
        assert_eq!(&buffer[..count], b"original bytes await the sole incident codec");
        assert!(std::fs::write(fixture.target.join(&fixture.name), b"changed").is_err());
        source.revalidate()?;
        assert_eq!(std::fs::read(fixture.original.path().join(&fixture.name))?, &buffer[..count]);
        source.retire(context)?;
        assert!(source.retired());
        assert!(source.revalidate().is_err());
        std::fs::write(
            fixture.target.join(&fixture.name),
            b"write succeeds only after checked pin closure",
        )?;
        assert_eq!(std::fs::read(fixture.original.path().join(&fixture.name))?, &buffer[..count]);
        Ok(())
    })
}

#[test]
fn native_passive_source_refuses_changed_copy_and_foreign_association_without_repairing_bytes()
-> TestResult {
    test_worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let fixture = Fixture::new(context)?;
        let foreign = InstallationId::new("f".repeat(32))?;
        assert!(
            PreservedDiagnosticSource::open_at(
                &fixture.root,
                imports::ImportAssociation::Installed(&foreign),
                fixture.namespace,
                context
            )
            .is_err()
        );
        std::fs::write(fixture.target.join(&fixture.name), b"later changed copy remains intact")?;
        assert!(fixture.open(context).is_err());
        assert_eq!(
            std::fs::read(fixture.target.join(&fixture.name))?,
            b"later changed copy remains intact"
        );
        assert_eq!(
            std::fs::read(fixture.original.path().join(&fixture.name))?,
            b"original bytes await the sole incident codec"
        );
        Ok(())
    })
}

#[test]
#[allow(unsafe_code)]
fn native_passive_source_failed_close_retains_exact_pins_until_checked_retry() -> TestResult {
    use windows::Win32::Foundation::{
        HANDLE_FLAG_PROTECT_FROM_CLOSE, HANDLE_FLAGS, SetHandleInformation,
    };
    test_worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let fixture = Fixture::new(context)?;
        let mut source = fixture.open(context)?;
        let raw = source.files.first().ok_or("Actual copied pin absent")?.file.raw();
        // SAFETY: This test exclusively owns the still-retained physical source handle. The
        // flag causes a real close refusal without changing any data or another process.
        unsafe {
            SetHandleInformation(
                raw,
                HANDLE_FLAG_PROTECT_FROM_CLOSE.0,
                HANDLE_FLAG_PROTECT_FROM_CLOSE,
            )
        }?;
        let failure = source.retire(context).err();
        let retained =
            std::fs::write(fixture.target.join(&fixture.name), b"must remain pinned").is_err();
        // SAFETY: A refused CloseHandle leaves this identical owned handle valid; clearing
        // only the fixture flag allows its original owner to perform checked retry.
        let cleared =
            unsafe { SetHandleInformation(raw, HANDLE_FLAG_PROTECT_FROM_CLOSE.0, HANDLE_FLAGS(0)) };
        cleared?;
        assert!(failure.is_some());
        assert!(!source.retired());
        assert!(retained);
        source.retire(context)?;
        assert!(source.retired());
        assert_eq!(
            std::fs::read(fixture.original.path().join(&fixture.name))?,
            b"original bytes await the sole incident codec"
        );
        Ok(())
    })
}

#[test]
fn native_passive_source_refuses_every_read_once_its_original_operation_expires() -> TestResult {
    test_worker::run_reserved_expiring(32 * 1024 * 1024, |context, clock| {
        let fixture = Fixture::new(context)?;
        let mut source = fixture.open(context)?;
        source.revalidate()?;
        clock.expire();
        let mut buffer = [0xa5; 64];
        assert!(source.read_chunk(0, 0, &mut buffer).is_err());
        assert_eq!(buffer, [0xa5; 64]);
        assert!(source.revalidate().is_err());
        assert!(source.matches(0, DiagnosticObject::StoreState).is_err());
        assert_eq!(
            std::fs::read(fixture.original.path().join(&fixture.name))?,
            b"original bytes await the sole incident codec"
        );
        Ok(())
    })
}
