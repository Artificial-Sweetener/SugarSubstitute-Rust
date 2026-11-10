// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pin the actual running image and primary account before the first passive storage write.
use crate::durable_storage::{
    check,
    files::{self, Directory},
    invalid, protection,
};
use serde::{Deserialize, Serialize};
use std::{fs::File, io, path::Path};
use sugarsubstitute_configuration::layout::InstallationLayout;
use sugarsubstitute_contracts::{execution::JobContext, lifecycle::NamespaceId};

/// Observed producer matching facts; serialized values never authorize native effects.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Producer {
    pub(super) namespace: [u8; 32],
    pub(super) account_sid: Vec<u8>,
    pub(super) session: u32,
    pub(super) process: u32,
    pub(super) creation_100ns: u64,
    pub(super) image: [u8; 24],
    pub(super) image_directory: [u8; 24],
    pub(super) root: [u8; 24],
}
impl Producer {
    pub(super) fn namespace(&self) -> NamespaceId {
        NamespaceId::new(self.namespace)
    }
}

/// Actual pins retain image/root ancestry; no claims, selection or backend ownership is minted.
pub(in crate::setup_storage::user_data) struct VerifiedHost {
    pub(super) root: Directory,
    pub(super) producer: Producer,
    _image: File,
    _image_directory: Directory,
}
impl VerifiedHost {
    /// Compare the expected image directory with the actual executable's pinned parent.
    /// The first account/runtime creation is permitted only after this function succeeds.
    pub(in crate::setup_storage::user_data) fn capture(
        expected: &Path,
        context: &JobContext,
    ) -> io::Result<Self> {
        check(context)?;
        Self::pin_image(expected, &std::env::current_exe()?, context)
    }
    fn pin_image(expected: &Path, current: &Path, context: &JobContext) -> io::Result<Self> {
        check(context)?;
        if context.deadline().is_none() {
            return Err(invalid("Invocation capture requires its original finite deadline"));
        }
        let parent = current.parent().ok_or_else(|| invalid("Current executable parent absent"))?;
        let image_directory = Directory::open(parent, false)?;
        let expected = Directory::open(expected, false)?;
        if expected.native_identity()? != image_directory.native_identity()? {
            return Err(invalid("Invocation image directory differs from actual running image"));
        }
        let name = current
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| invalid("Current executable name is not Unicode"))?;
        let image = image_directory.read(name)?;
        let image_identity = files::native_identity(&image)?;
        let layout = InstallationLayout::for_image_directory(image_directory.path());
        let root = Directory::open(layout.root(), false)?;
        if !image_directory.has_ancestor(&root, context)? {
            return Err(invalid("Invocation layout root differs from pinned image ancestry"));
        }
        let process = crate::native_process::current_identity(context).map_err(io::Error::other)?;
        let producer = protection::with(context, |owner| {
            let mut account_sid = Vec::new();
            account_sid.try_reserve_exact(owner.account_sid().len()).map_err(io::Error::other)?;
            account_sid.extend_from_slice(owner.account_sid());
            Ok(Producer {
                namespace: *owner.generation_namespace(&image_directory, context)?.bytes(),
                account_sid,
                session: owner.session_id(),
                process: process.pid,
                creation_100ns: process.birth,
                image: image_identity,
                image_directory: image_directory.native_identity()?,
                root: root.native_identity()?,
            })
        })?;
        check(context)?;
        Ok(Self { root, producer, _image: image, _image_directory: image_directory })
    }
    /// Substitute only the current-executable query; filesystem/account/process facts stay native.
    #[cfg(test)]
    pub(super) fn fixture(image: &Path, context: &JobContext) -> io::Result<Self> {
        Self::pin_image(
            image.parent().ok_or_else(|| invalid("Fixture image parent absent"))?,
            image,
            context,
        )
    }
    /// Create only the fixed protected feature namespace after current host/account verification.
    pub(super) fn originals_directory(
        &self,
        create: bool,
        context: &JobContext,
    ) -> io::Result<Directory> {
        protection::with(context, |owner| {
            if owner.account_sid() != self.producer.account_sid
                || owner.session_id() != self.producer.session
            {
                return Err(invalid("Invocation primary account/session changed"));
            }
            let layout = InstallationLayout::new(self.root.path());
            let path =
                owner.runtime_storage(&layout.user(), "entry-originals-v1", create, context)?;
            let directory = Directory::open(&path, false)?.resolved()?;
            if !directory.has_ancestor(&self.root, context)? {
                return Err(invalid("Passive originals escaped their actual pinned root"));
            }
            Ok(directory)
        })
    }
}
