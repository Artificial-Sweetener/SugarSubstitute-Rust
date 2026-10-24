// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Anchor Unix artifact effects to retained directory descriptors and observe close outcomes.
use super::*;
use std::{
    ffi::CString,
    fs::File,
    os::{
        fd::{AsRawFd, FromRawFd, IntoRawFd},
        unix::ffi::OsStrExt,
    },
    sync::Arc,
};

/// One exclusively owned descriptor. Uncertain close is never retried against a reusable fd.
struct Descriptor {
    file: Option<File>,
    memory: Arc<MemoryClaim>,
}
impl Descriptor {
    /// Adopt a successful native return before observing later controls.
    fn adopt(raw: libc::c_int, memory: Arc<MemoryClaim>) -> io::Result<Self> {
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: A successful openat returns this new, exclusively owned descriptor.
        let file = unsafe { File::from_raw_fd(raw) };
        Ok(Self { file: Some(file), memory })
    }
    fn file(&self) -> io::Result<&File> {
        self.file.as_ref().ok_or_else(|| invalid("Artifact descriptor is closed"))
    }
    fn close(&mut self) -> io::Result<()> {
        let Some(file) = self.file.take() else { return Ok(()) };
        let raw = file.into_raw_fd();
        // SAFETY: Ownership was transferred out of File exactly once. A failed close has
        // platform-dependent descriptor state, so this owner never retries that number.
        if unsafe { libc::close(raw) } != 0 {
            let cause = io::Error::last_os_error();
            std::mem::forget(self.memory.clone());
            return Err(cause);
        }
        Ok(())
    }
}
impl Drop for Descriptor {
    fn drop(&mut self) {
        if let Err(cause) = self.close() {
            tracing::error!(operation="artifact_descriptor_retirement", error=%cause, "Artifact descriptor retirement is uncertain");
        }
    }
}

pub(super) struct Stage {
    file: Descriptor,
    parent: Descriptor,
    staged: CString,
    target: CString,
}
pub(super) struct Input(Descriptor);
impl Input {
    pub(super) fn open(path: &Path, context: &JobContext) -> io::Result<Self> {
        check(context)?;
        let memory = Arc::new(context.memory().claim(64 * 1024).map_err(io::Error::other)?);
        let result = File::open(path).map(|file| Self(Descriptor { file: Some(file), memory }));
        super::super::control::observe(context, result)
    }
    pub(super) fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.0.file()?.read(bytes)
    }
    pub(super) fn close(&mut self) -> io::Result<()> {
        self.0.close()
    }
}
impl Stage {
    pub(super) fn open(path: &Path, create: bool, context: &JobContext) -> io::Result<Self> {
        let memory = Arc::new(context.memory().claim(64 * 1024).map_err(io::Error::other)?);
        let target = output_path(path)?;
        let parent_path = target.parent().ok_or_else(|| invalid("Artifact parent missing"))?;
        let root = CString::new("/").map_err(io::Error::other)?;
        check(context)?;
        // SAFETY: The terminated root string is live; no creation mode is required.
        let raw = unsafe {
            libc::open(root.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC)
        };
        let mut parent = Descriptor::adopt(raw, memory.clone())?;
        check(context)?;
        for component in parent_path.components() {
            let std::path::Component::Normal(name) = component else { continue };
            let name = CString::new(name.as_bytes()).map_err(io::Error::other)?;
            let fd = parent.file()?.as_raw_fd();
            if create {
                effect(context, || {
                    // SAFETY: The held directory and terminated flat leaf remain live.
                    let result = unsafe { libc::mkdirat(fd, name.as_ptr(), 0o777) };
                    if result == 0 {
                        return Ok(());
                    }
                    let cause = io::Error::last_os_error();
                    if cause.kind() == io::ErrorKind::AlreadyExists { Ok(()) } else { Err(cause) }
                })?;
            }
            check(context)?;
            // SAFETY: Both operands remain live. NOFOLLOW rejects every redirected child;
            // later effects use this actual descriptor even if its ancestry is renamed.
            let raw = unsafe {
                libc::openat(
                    fd,
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            let next = Descriptor::adopt(raw, memory.clone())?;
            parent.close()?;
            parent = next;
            check(context)?;
        }
        let leaf = target.file_name().ok_or_else(|| invalid("Artifact leaf missing"))?;
        let target = CString::new(leaf.as_bytes()).map_err(io::Error::other)?;
        for index in 0..128 {
            let staged = CString::new(format!(".artifact-{}-{index}", std::process::id()))
                .map_err(io::Error::other)?;
            check(context)?;
            // SAFETY: Parent and terminated flat leaf remain live. EXCL refuses every
            // occupied name; NOFOLLOW and CLOEXEC preserve descriptor custody.
            let raw = unsafe {
                libc::openat(
                    parent.file()?.as_raw_fd(),
                    staged.as_ptr(),
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC,
                    0o666,
                )
            };
            match Descriptor::adopt(raw, memory.clone()) {
                Ok(file) => {
                    check(context)?;
                    return Ok(Self { parent, staged, target, file });
                }
                Err(cause) if cause.kind() == io::ErrorKind::AlreadyExists => {}
                Err(cause) => return Err(cause),
            }
        }
        Err(invalid("Artifact staging namespace is occupied"))
    }
    pub(super) fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.file.file()?.write(bytes)
    }
    pub(super) fn flush(&mut self) -> io::Result<()> {
        self.file.file()?.sync_all()
    }
    pub(super) fn close(&mut self) -> io::Result<()> {
        self.file.close()
    }
    pub(super) fn abort(&mut self) -> io::Result<()> {
        finish_io(self.close(), self.parent.close())
    }
    pub(super) fn publish(
        mut self,
        mode: ArtifactPublication,
        context: &JobContext,
    ) -> io::Result<()> {
        let fd = self.parent.file()?.as_raw_fd();
        let published = effect(context, || {
            // SAFETY: Both flat names are terminated, and both directories are the same
            // retained descriptor. No path traversal or target-link following occurs.
            let result = unsafe {
                match mode {
                    ArtifactPublication::CreateOnly => {
                        libc::linkat(fd, self.staged.as_ptr(), fd, self.target.as_ptr(), 0)
                    }
                    ArtifactPublication::ReplaceSelected => {
                        libc::renameat(fd, self.staged.as_ptr(), fd, self.target.as_ptr())
                    }
                }
            };
            if result != 0 {
                return Err(io::Error::last_os_error());
            }
            if matches!(mode, ArtifactPublication::CreateOnly) {
                // SAFETY: Only this newly created stage name is removed after linking it.
                if unsafe { libc::unlinkat(fd, self.staged.as_ptr(), 0) } != 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        });
        finish_io(published, self.parent.close())
    }
}
