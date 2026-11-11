// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Create desktop release inputs through the existing protected-storage security policy.
use super::check;
use crate::{
    ClaimError,
    durable_storage::{
        files::{self, Directory},
        protected::{
            account, directories, native_name::NativeNameDestination, native_phase::NativePhase,
            paths, security,
        },
    },
    native_claims::files::{Budget, Handle},
};
use std::{
    fs::File,
    io,
    os::windows::{
        fs::MetadataExt,
        io::{AsRawHandle, FromRawHandle},
    },
    path::Path,
    sync::Arc,
};
use sugarsubstitute_contracts::execution::JobContext;
use windows::Win32::{Foundation::HANDLE, Storage::FileSystem::*};

mod exclusion;
mod outcome;
mod publication;
mod retirement;
mod runtime_storage;
mod stage;
pub(crate) use outcome::{DomainFailure, with_domain};
pub(crate) use publication::Publication;

/// Retain account and directory custody across one admitted installation preparation.
pub(crate) struct Protection {
    handles: Vec<Handle>,
    sid: Vec<u8>,
    session_id: u32,
    budget: Budget,
}

/// Observe cleanup even when payload creation fails; no permission repair touches existing files.
pub(crate) fn with<T>(
    context: &JobContext,
    operation: impl FnOnce(&mut Protection) -> io::Result<T>,
) -> io::Result<T> {
    let _operation =
        tracing::trace_span!(target: "sugarsubstitute::storage", "protected_operation").entered();
    check(context)?;
    let budget =
        Budget::Protected(Arc::new(context.memory().claim(1024 * 1024).map_err(io::Error::other)?));
    let mut owner =
        Protection { handles: Vec::with_capacity(260), sid: Vec::new(), session_id: 0, budget };
    let result = (|| {
        let mut phase = NativePhase::ordinary(&owner.budget)?;
        let account = {
            let _account =
                tracing::trace_span!(target: "sugarsubstitute::storage", "protection_account")
                    .entered();
            account::resolve_current(
                &mut owner.handles,
                owner.budget.clone(),
                account::PrimaryAccountAccess::Query,
                &mut phase,
            )?
        };
        owner.sid = account.sid.bytes()?.to_vec();
        owner.session_id = account.session_id;
        operation(&mut owner).map_err(|cause| ClaimError::new("setup_protected_payload", cause))
    })();
    let _retirement =
        tracing::trace_span!(target: "sugarsubstitute::storage", "protection_retirement").entered();
    let mut cleanup = Ok(());
    for handle in owner.handles.iter_mut().rev() {
        cleanup = ClaimError::finish(cleanup, handle.close());
    }
    ClaimError::finish(result, cleanup).map_err(io::Error::other)
}

impl Protection {
    /// Borrow the actual retained primary-account SID for an existing native record codec.
    /// Configured paths, environment variables and presentation never supply this identity.
    pub(crate) fn account_sid(&self) -> &[u8] {
        &self.sid
    }
    /// Observe the primary token's Windows session captured with this account custody.
    pub(crate) fn session_id(&self) -> u32 {
        self.session_id
    }

    /// Reuse the unchanged namespace hash with this actual account and an already pinned generation.
    /// The caller admits that generation through protected installation selection before importing.
    pub(crate) fn generation_namespace(
        &self,
        generation: &Directory,
        context: &JobContext,
    ) -> io::Result<sugarsubstitute_contracts::lifecycle::NamespaceId> {
        check(context)?;
        let namespace = account::namespace_identity(&self.sid, &generation.native_identity()?);
        check(context)?;
        Ok(namespace)
    }

    /// Check an already pinned file against the same account-specific protection policy.
    pub(crate) fn verify_file(&self, file: &File) -> io::Result<()> {
        let _verification =
            tracing::trace_span!(target: "sugarsubstitute::storage", "protection_file_verify")
                .entered();
        let mut phase = NativePhase::ordinary(&self.budget).map_err(io::Error::other)?;
        security::verify_borrowed(HANDLE(file.as_raw_handle()), &self.budget, &self.sid, &mut phase)
            .map_err(io::Error::other)
    }

    /// Atomically create and retain a protected child under its pinned canonical parent.
    pub(crate) fn directory(
        &mut self,
        path: &Path,
        mode: directories::Mode,
        context: &JobContext,
    ) -> io::Result<directories::DirectoryDisposition> {
        let _directory =
            tracing::trace_span!(target: "sugarsubstitute::storage", "protection_directory")
                .entered();
        check(context)?;
        let parent = path.parent().ok_or_else(|| super::invalid("Release parent absent"))?;
        let _ancestors = Directory::open(parent, false)?;
        let result = (|| {
            let mut phase = NativePhase::ordinary(&self.budget)?;
            let parent = paths::open_retained(
                parent,
                true,
                false,
                self.budget.clone(),
                &mut self.handles,
                &mut phase,
            )?;
            let canonical = paths::final_path(
                &self.handles[parent],
                &mut phase,
                NativeNameDestination::Ordinary,
            )?;
            let canonical = canonical
                .path()?
                .join(path.file_name().ok_or_else(|| ClaimError::invalid("setup_release_leaf"))?);
            let opened = directories::open_relative(
                &mut self.handles,
                &self.sid,
                self.budget.clone(),
                directories::RelativeDirectory { path: &canonical, parent, mode, outputs: None },
                context,
                &mut phase,
            )?;
            Ok::<_, ClaimError>(opened.disposition)
        })();
        result.map_err(io::Error::other)
    }

    /// Create a new file with explicit release protection, verified on the same open handle.
    pub(crate) fn file(&self, path: &Path, context: &JobContext) -> io::Result<File> {
        self.open_file(path, CREATE_NEW, FILE_SHARE_MODE(0), context)
    }

    /// Create an owned transfer snapshot whose last native handle closure removes its name.
    /// CREATE_NEW preserves unknown files; the caller pins the parent through creation.
    pub(crate) fn transient_file(&self, path: &Path, context: &JobContext) -> io::Result<File> {
        self.open_file_with_lifetime(
            path,
            CREATE_NEW,
            FILE_SHARE_READ | FILE_SHARE_DELETE,
            FILE_FLAG_DELETE_ON_CLOSE,
            context,
        )
    }

    /// Open an empty protected OS-lock rendezvous without repairing existing permissions.
    /// The caller pins the parent. Existence is never evidence that an owner is running.
    pub(crate) fn rendezvous(&self, path: &Path, context: &JobContext) -> io::Result<File> {
        self.open_rendezvous(path, FILE_SHARE_READ | FILE_SHARE_WRITE, context)
    }

    /// Open the same shared rendezvous for overlapped I/O so the execution owner can wait for
    /// its byte-range lock with a deadline. The handle is used only for locking.
    pub(crate) fn waitable_rendezvous(
        &self,
        path: &Path,
        context: &JobContext,
    ) -> io::Result<File> {
        let file = self.open_file_with_lifetime(
            path,
            OPEN_ALWAYS,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            FILE_FLAG_OVERLAPPED,
            context,
        )?;
        if file.metadata()?.len() != 0 {
            return Err(super::invalid("Lock rendezvous contains unrecognized data"));
        }
        Ok(file)
    }

    /// Acquire exclusion attached to the native file object until its last handle closes.
    /// Duplicates retain this exclusion; opening an older shared rendezvous also conflicts.
    /// The caller pins the parent and retains custody through its entire owned lifetime.
    pub(crate) fn exclusive_rendezvous(
        &self,
        path: &Path,
        context: &JobContext,
    ) -> io::Result<File> {
        self.open_rendezvous(path, FILE_SHARE_MODE(0), context)
    }

    /// Validate the empty rendezvous on the same protected handle that owns its sharing mode.
    fn open_rendezvous(
        &self,
        path: &Path,
        sharing: FILE_SHARE_MODE,
        context: &JobContext,
    ) -> io::Result<File> {
        let file = self.open_file(path, OPEN_ALWAYS, sharing, context)?;
        if file.metadata()?.len() != 0 {
            return Err(super::invalid("Lock rendezvous contains unrecognized data"));
        }
        Ok(file)
    }

    /// Apply protection only on creation; validate existing objects through their exact handles.
    fn open_file(
        &self,
        path: &Path,
        creation: FILE_CREATION_DISPOSITION,
        sharing: FILE_SHARE_MODE,
        context: &JobContext,
    ) -> io::Result<File> {
        self.open_file_with_lifetime(path, creation, sharing, FILE_FLAGS_AND_ATTRIBUTES(0), context)
    }

    /// Keep native deletion lifetime and I/O mode explicit while sharing one descriptor and
    /// verification path.
    fn open_file_with_lifetime(
        &self,
        path: &Path,
        creation: FILE_CREATION_DISPOSITION,
        sharing: FILE_SHARE_MODE,
        flags: FILE_FLAGS_AND_ATTRIBUTES,
        context: &JobContext,
    ) -> io::Result<File> {
        let delete_on_close = flags.0 & FILE_FLAG_DELETE_ON_CLOSE.0 != 0;
        check(context)?;
        let _file =
            tracing::trace_span!(target: "sugarsubstitute::storage", "protection_file_open")
                .entered();
        let mut phase = NativePhase::ordinary(&self.budget).map_err(io::Error::other)?;
        let descriptor =
            security::Descriptor::new(&self.sid, &mut phase).map_err(io::Error::other)?;
        let attributes = descriptor.attributes();
        // SAFETY: The caller pins the parent. Both allowed dispositions preserve existing bytes.
        // The descriptor and path remain live, and File adopts the sole returned handle immediately.
        let raw = unsafe {
            CreateFileW(
                &files::native_path(path),
                FILE_GENERIC_READ.0
                    | FILE_GENERIC_WRITE.0
                    | if delete_on_close { DELETE.0 } else { 0 },
                sharing,
                Some(&attributes),
                creation,
                FILE_FLAG_OPEN_REPARSE_POINT | flags,
                None,
            )
        }
        .map_err(io::Error::other)?;
        // SAFETY: Successful CreateFileW returned this unique valid owned file handle.
        let file = unsafe { File::from_raw_handle(raw.0) };
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 {
            return Err(super::invalid("Protected storage requires a regular file"));
        }
        {
            let _verification =
                tracing::trace_span!(target: "sugarsubstitute::storage", "protection_file_verify")
                    .entered();
            security::verify_borrowed(
                HANDLE(file.as_raw_handle()),
                &self.budget,
                &self.sid,
                &mut phase,
            )
            .map_err(io::Error::other)?;
        }
        check(context)?;
        Ok(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_claims::files::test_support::{TestResult, admitted};
    use std::io::Write;

    #[test]
    fn installed_inputs_satisfy_runtime_protection_without_repairing_foreign_content() -> TestResult
    {
        admitted(|context| {
            let temporary = tempfile::tempdir().map_err(|e| ClaimError::new("fixture", e))?;
            let generation = temporary.path().join("release");
            let capture = generation.join("capture");
            let unknown = temporary.path().join("unknown");
            std::fs::create_dir(&unknown).map_err(|e| ClaimError::new("fixture", e))?;
            with(&context, |protection| {
                protection.directory(&generation, directories::Mode::CreateNew, &context)?;
                protection.directory(&capture, directories::Mode::CreateNew, &context)?;
                let payload = generation.join("payload");
                protection.file(&payload, &context)?.write_all(b"original")?;
                assert!(protection.file(&payload, &context).is_err());
                assert_eq!(std::fs::read(payload)?, b"original");
                assert!(
                    protection
                        .directory(&unknown, directories::Mode::OpenOrCreate, &context,)
                        .is_err()
                );
                assert!(std::fs::read_dir(&unknown)?.next().is_none());
                Ok(())
            })
            .map_err(|e| ClaimError::new("fixture_protection", e))?;
            let memory = context
                .memory()
                .claim(1024 * 1024)
                .map_err(|e| ClaimError::new("fixture_memory", e))?;
            let mut runtime = crate::ProtectedStorageRoot::new(memory)?;
            let checked = runtime.resolve(&capture, &context);
            let cleanup = runtime.retire(&context);
            ClaimError::finish(checked, cleanup)
        })
    }
}
