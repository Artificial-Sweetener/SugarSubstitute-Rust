// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Derive namespace children from each actual retained protected directory's final name.
use super::super::security;
use super::*;
/// Fixed account runtime roots below an installation's `runtime` folder; the final native name
/// determines each following child.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Namespace {
    /// Durable process ownership and incident claims.
    Claims,
    /// A validated account-runtime child whose persisted name belongs to its feature owner.
    RuntimeChild(&'static str),
}
/// Resolve and validate a fixed namespace under the retained installation runtime base.
/// Creation uses the account storage boundary; existing-only access preserves absent names.
/// The native boundary can redirect a logical directory name. Each next child and
/// every returned claim stem uses the final name of that actual opened directory.
/// # Errors
/// Native creation, protection and canonical-name errors preserve adopted handles.
pub(crate) fn resolve(
    directory: PathBuf,
    namespace: Namespace,
    create: bool,
    sid: &[u8],
    budget: Budget,
    handles: &mut Vec<Handle>,
) -> Result<(PathBuf, usize, [u8; 24]), ClaimError> {
    let mut native = NativeDirectory { sid, budget, handles, create, count: 0, namespace: None };
    let directory = resolve_with_boundary(directory, namespace, &mut native)?;
    let (index, identity) =
        native.namespace.ok_or_else(|| ClaimError::invalid("namespace_directory_pin"))?;
    Ok((directory, index, identity))
}
/// Adapter boundary for actual creation, retained handle adoption and final-name observation.
trait DirectoryBoundary {
    fn create_and_observe(&mut self, path: &Path) -> Result<PathBuf, ClaimError>;
}
/// Components from the runtime base to the pinned namespace directory: `runtime\<leaf>`.
const NAMESPACE_DEPTH: usize = 2;
struct NativeDirectory<'a> {
    sid: &'a [u8],
    budget: Budget,
    handles: &'a mut Vec<Handle>,
    create: bool,
    count: usize,
    namespace: Option<(usize, [u8; 24])>,
}
impl DirectoryBoundary for NativeDirectory<'_> {
    fn create_and_observe(&mut self, path: &Path) -> Result<PathBuf, ClaimError> {
        let index = self.handles.len();
        let mut phase = NativePhase::ordinary(&self.budget)?;
        if self.create {
            paths::create_directory(path, self.sid, self.budget.clone(), self.handles, &mut phase)?;
        } else {
            let opened = paths::open_retained(
                path,
                true,
                true,
                self.budget.clone(),
                self.handles,
                &mut phase,
            )?;
            security::verify(&self.handles[opened], self.sid, &mut phase)?;
        }
        let handle = self
            .handles
            .get(index)
            .ok_or_else(|| ClaimError::invalid("namespace_directory_handle"))?;
        self.count += 1;
        if self.count == NAMESPACE_DEPTH {
            self.namespace = Some((index, paths::file_identity(handle, &mut phase)?));
        }
        paths::final_path(
            handle,
            &mut phase,
            crate::durable_storage::protected::native_name::NativeNameDestination::Ordinary,
        )?
        .into_ordinary()
    }
}
fn resolve_with_boundary(
    mut directory: PathBuf,
    namespace: Namespace,
    boundary: &mut dyn DirectoryBoundary,
) -> Result<PathBuf, ClaimError> {
    let leaf = match namespace {
        Namespace::Claims => "claims",
        Namespace::RuntimeChild(leaf) => {
            crate::durable_storage::files::flat_leaf(leaf)
                .map_err(|cause| ClaimError::new("account_namespace_leaf", cause))?;
            leaf
        }
    };
    for component in [sugarsubstitute_configuration::layout::RUNTIME, leaf] {
        directory = boundary.create_and_observe(&directory.join(component))?;
    }
    if namespace == Namespace::Claims {
        for component in ["coordinators", "backends"] {
            boundary.create_and_observe(&directory.join(component))?;
        }
    }
    Ok(directory)
}
#[cfg(test)]
mod tests;
