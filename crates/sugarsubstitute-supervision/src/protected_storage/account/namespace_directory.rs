// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Derive namespace children from each actual retained protected directory's final name.
use super::*;
/// Create and validate the namespace directories under the retained account root.
/// The native boundary can redirect a logical directory name. Each next child and
/// every returned claim stem uses the final name of that actual opened directory.
/// # Errors
/// Native creation, protection and canonical-name errors preserve adopted handles.
pub(super) fn resolve(
    directory: PathBuf,
    sid: &[u8],
    budget: Budget,
    handles: &mut Vec<Handle>,
) -> Result<(PathBuf, usize, [u8; 24]), ClaimError> {
    let mut native = NativeDirectory { sid, budget, handles, count: 0, namespace: None };
    let directory = resolve_with_boundary(directory, &mut native)?;
    let (index, identity) =
        native.namespace.ok_or_else(|| ClaimError::invalid("namespace_directory_pin"))?;
    Ok((directory, index, identity))
}
/// Adapter boundary for actual creation, retained handle adoption and final-name observation.
trait DirectoryBoundary {
    fn create_and_observe(&mut self, path: &Path) -> Result<PathBuf, ClaimError>;
}
struct NativeDirectory<'a> {
    sid: &'a [u8],
    budget: Budget,
    handles: &'a mut Vec<Handle>,
    count: usize,
    namespace: Option<(usize, [u8; 24])>,
}
impl DirectoryBoundary for NativeDirectory<'_> {
    fn create_and_observe(&mut self, path: &Path) -> Result<PathBuf, ClaimError> {
        let index = self.handles.len();
        let mut phase = NativePhase::ordinary(&self.budget)?;
        paths::create_directory(path, self.sid, self.budget.clone(), self.handles, &mut phase)?;
        let handle = self
            .handles
            .get(index)
            .ok_or_else(|| ClaimError::invalid("namespace_directory_handle"))?;
        self.count += 1;
        if self.count == 3 {
            self.namespace = Some((index, paths::file_identity(handle, &mut phase)?));
        }
        paths::final_path(
            handle,
            &mut phase,
            crate::protected_storage::native_name::NativeNameDestination::Ordinary,
        )?
        .into_ordinary()
    }
}
fn resolve_with_boundary(
    mut directory: PathBuf,
    boundary: &mut dyn DirectoryBoundary,
) -> Result<PathBuf, ClaimError> {
    for component in ["SugarSubstitute", "runtime", "claims"] {
        directory = boundary.create_and_observe(&directory.join(component))?;
    }
    for component in ["coordinators", "backends"] {
        boundary.create_and_observe(&directory.join(component))?;
    }
    Ok(directory)
}
#[cfg(test)]
mod tests;
