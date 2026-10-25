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
use std::{os::windows::io::IntoRawHandle, sync::Arc};

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
impl Input {
    pub(super) fn open(path: &Path, context: &JobContext) -> io::Result<Self> {
        check(context)?;
        let budget = Budget::Protected(Arc::new(
            context.memory().claim(64 * 1024).map_err(io::Error::other)?,
        ));
        let result = std::fs::File::open(path)
            .map(|file| Self(Handle::new(HANDLE(file.into_raw_handle()), budget)));
        super::super::control::observe(context, result)
    }
    pub(super) fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let file = self.0.borrow_file().map_err(io::Error::other)?;
        (&*file).read(bytes)
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
