// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain Windows artifact ancestry and observe actual file closure before publication.
use super::*;
use crate::{
    durable_storage::{
        files::{self, Directory},
        staging,
    },
    native_claims::files::{Budget, Handle},
};
use ::windows::Win32::Foundation::HANDLE;
use std::{
    io::{Seek, SeekFrom},
    os::windows::{fs::OpenOptionsExt, io::IntoRawHandle},
    sync::Arc,
};

pub(super) struct Stage {
    file: Handle,
    parent: Parent,
    staged: PathBuf,
    target: PathBuf,
}

struct Parent {
    directory: Option<Directory>,
    budget: Budget,
}
impl Parent {
    fn directory(&self) -> io::Result<&Directory> {
        self.directory.as_ref().ok_or_else(|| invalid("Artifact parent is retired"))
    }
    fn close(&mut self) -> io::Result<()> {
        match self.directory.take() {
            Some(directory) => directory.retire(&self.budget),
            None => Ok(()),
        }
    }
}
impl Drop for Parent {
    fn drop(&mut self) {
        if let Err(cause) = self.close() {
            tracing::error!(operation="artifact_parent_retirement", error=%cause, "Artifact parent retirement failed");
        }
    }
}
pub(super) struct Input(Handle);

/// One immutable source together with every native ancestry pin that selected it.
pub(super) struct RetainedInput {
    file: Input,
    parent: Parent,
    failed_parent: Option<Arc<io::Error>>,
}
impl RetainedInput {
    pub(super) fn open(
        path: &Path,
        memory: &sugarsubstitute_contracts::execution::MemoryLease,
        context: &JobContext,
    ) -> io::Result<Self> {
        files::lexical(path)?;
        let budget =
            Budget::Protected(Arc::new(memory.claim(64 * 1024).map_err(io::Error::other)?));
        let parent = effect(context, || {
            Directory::open(
                path.parent().ok_or_else(|| invalid("Retained source parent absent"))?,
                false,
            )
        })?;
        let parent = Parent { directory: Some(parent), budget: budget.clone() };
        let file = effect(context, || {
            std::fs::OpenOptions::new().read(true).share_mode(1).custom_flags(0x0020_0000).open(
                parent
                    .directory()?
                    .path()
                    .join(path.file_name().ok_or_else(|| invalid("Retained source leaf absent"))?),
            )
        })?;
        let input = Input(Handle::new(HANDLE(file.into_raw_handle()), budget));
        let mut result = Self { file: input, parent, failed_parent: None };
        let validation = (|| {
            use std::os::windows::fs::MetadataExt;
            let file = result.file.0.borrow_file().map_err(io::Error::other)?;
            let metadata = file.metadata()?;
            if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 {
                return Err(invalid(
                    "Retained source must be an ordinary file without redirection",
                ));
            }
            files::single_link(&file)?;
            check(context)
        })();
        match validation {
            Ok(()) => Ok(result),
            Err(cause) => Err(retirement_error(cause, result.close())),
        }
    }
    pub(super) fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.file.read(bytes)
    }
    pub(super) fn length(&mut self) -> io::Result<u64> {
        self.file.length()
    }
    pub(super) fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.file.seek(position)
    }
    pub(super) fn close(&mut self) -> io::Result<()> {
        if let Some(cause) = &self.failed_parent {
            return Err(io::Error::other(ParentRetirement(cause.clone())));
        }
        self.file.close()?;
        if let Err(cause) = self.parent.close() {
            let cause = Arc::new(cause);
            self.failed_parent = Some(cause.clone());
            return Err(io::Error::other(ParentRetirement(cause)));
        }
        Ok(())
    }
}
#[derive(Debug)]
struct ParentRetirement(Arc<io::Error>);
impl std::fmt::Display for ParentRetirement {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Retained artifact ancestry did not retire")
    }
}
impl std::error::Error for ParentRetirement {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.0.as_ref())
    }
}
impl Input {
    pub(super) fn open(path: &Path, context: &JobContext) -> io::Result<Self> {
        check(context)?;
        let budget = Budget::Protected(Arc::new(
            context.memory().claim(64 * 1024).map_err(io::Error::other)?,
        ));
        let result = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(path)
            .map(|file| Self(Handle::new(HANDLE(file.into_raw_handle()), budget)));
        super::super::control::observe(context, result)
    }
    pub(super) fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let file = self.0.borrow_file().map_err(io::Error::other)?;
        (&*file).read(bytes)
    }
    pub(super) fn length(&mut self) -> io::Result<u64> {
        let file = self.0.borrow_file().map_err(io::Error::other)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(invalid("Selected input must be a regular file"));
        }
        Ok(metadata.len())
    }
    pub(super) fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let file = self.0.borrow_file().map_err(io::Error::other)?;
        (&*file).seek(position)
    }
    pub(super) fn close(&mut self) -> io::Result<()> {
        self.0.close().map_err(io::Error::other)
    }
}
impl Stage {
    pub(super) fn open(path: &Path, create: bool, context: &JobContext) -> io::Result<Self> {
        let budget = Budget::Protected(Arc::new(
            context.memory().claim(64 * 1024).map_err(io::Error::other)?,
        ));
        let target = output_path(path)?;
        files::lexical(&target)?;
        check(context)?;
        let result = Directory::open(
            target.parent().ok_or_else(|| invalid("Artifact parent missing"))?,
            create,
        )
        .map(|directory| Parent { directory: Some(directory), budget: budget.clone() });
        let parent = super::super::control::observe(context, result)?;
        let staged = staging::unique_path(parent.directory()?, ".artifact-pending")?;
        check(context)?;
        let result = files::create_new(&staged)
            .map(|file| Handle::new(HANDLE(file.into_raw_handle()), budget));
        let file = super::super::control::observe(context, result)?;
        Ok(Self { parent, staged, target, file })
    }
    pub(super) fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let file = self.file.borrow_file().map_err(io::Error::other)?;
        (&*file).write(bytes)
    }
    pub(super) fn flush(&mut self) -> io::Result<()> {
        self.file.borrow_file().map_err(io::Error::other)?.sync_all()
    }
    pub(super) fn close(&mut self) -> io::Result<()> {
        self.file.close().map_err(io::Error::other)
    }
    pub(super) fn abort(&mut self) -> io::Result<()> {
        finish_io(self.close(), self.parent.close())
    }
    pub(super) fn publish(
        mut self,
        mode: ArtifactPublication,
        context: &JobContext,
    ) -> io::Result<()> {
        let published = effect(context, || match mode {
            ArtifactPublication::CreateOnly => staging::publish(&self.staged, &self.target),
            ArtifactPublication::ReplaceSelected => {
                staging::replace_record(&self.staged, &self.target)
            }
        });
        finish_io(published, self.parent.close())
    }
}

#[cfg(test)]
#[allow(unsafe_code)]
mod tests {
    use super::*;
    use ::windows::Win32::Foundation::{
        HANDLE_FLAG_PROTECT_FROM_CLOSE, HANDLE_FLAGS, SetHandleInformation,
    };

    #[test]
    fn selected_stream_excludes_external_writes_until_original_native_close()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        super::super::tests::run(|context, _| {
            let root = tempfile::tempdir()?;
            let path = root.path().join("authored");
            std::fs::write(&path, b"original")?;
            with_artifact_input(&path, 8, context, |_| {
                assert!(std::fs::write(&path, b"changed").is_err());
                Ok::<(), io::Error>(())
            })?;
            std::fs::write(&path, b"changed")?;
            assert_eq!(std::fs::read(path)?, b"changed");
            Ok(())
        })
    }

    /// Real native close failure keeps custody and ancestry until checked cleanup succeeds.
    #[test]
    fn failed_artifact_close_retains_the_original_file_and_pinned_parent()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        super::super::tests::run(|context, _| {
            let root = tempfile::tempdir()?;
            let parent = root.path().join("parent");
            std::fs::create_dir(&parent)?;
            let target = parent.join("receipt");
            std::fs::write(&target, b"preserved")?;
            let mut stage = Stage::open(&target, false, context)?;
            stage.write(b"pending")?;
            let raw = stage.file.raw();
            // SAFETY: The stage exclusively owns this live handle; the fixture clears
            // exactly this flag before asking its original owner to retire it again.
            unsafe {
                SetHandleInformation(
                    raw,
                    HANDLE_FLAG_PROTECT_FROM_CLOSE.0,
                    HANDLE_FLAG_PROTECT_FROM_CLOSE,
                )
            }
            .map_err(io::Error::other)?;
            let failed = stage.close().is_err();
            let ancestry_retained = std::fs::rename(&parent, root.path().join("moved")).is_err();
            let file_retained = std::fs::File::open(&stage.staged).is_err();
            // SAFETY: The failed close leaves the stage's protected handle live and owned.
            unsafe { SetHandleInformation(raw, HANDLE_FLAG_PROTECT_FROM_CLOSE.0, HANDLE_FLAGS(0)) }
                .map_err(io::Error::other)?;
            stage.abort()?;
            drop(stage);
            assert!(failed && ancestry_retained && file_retained);
            assert_eq!(std::fs::read(&target)?, b"preserved");
            std::fs::rename(&parent, root.path().join("moved"))?;
            let full = context.memory().claim(context.memory().bytes())?;
            drop(full);
            Ok(())
        })
    }
}
