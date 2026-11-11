// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind the matched worker's version-two model integration to actual native backend provenance.
use super::*;
use crate::durable_storage::user_paths;
use sugarsubstitute_contracts::execution::MemoryClaim;

/// Inert matched-worker payload. Values express the operation; private native custody admits it.
/// Envelope one carries integration version two. Earlier integration shapes are rejected before
/// dependency installation or model configuration; ReadyRecord schema-one bytes stay unchanged.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationRequest {
    version: u32,
    #[serde(flatten)]
    runtime: RuntimePaths,
    storage_base: PathBuf,
    models: PathBuf,
}
/// Actual root/source/provisioning inputs remain pinned through the worker's terminal result.
/// The request exposes no file handle, ownership constructor or filesystem grant.
pub struct IntegrationStorage {
    request: IntegrationRequest,
    _root: Directory,
    _source: Directory,
    _storage: Option<Directory>,
    _inputs: Vec<File>,
    _memory: MemoryClaim,
}
impl IntegrationStorage {
    /// Serialize only inert intent on the already authenticated matched-worker transport.
    pub fn request(&self) -> &IntegrationRequest {
        &self.request
    }
    /// Refuse a terminal response that changes the native-admitted runtime before publication.
    /// # Errors
    /// Returns an original-input mismatch without adopting the worker's changed paths.
    pub fn validate_result(&self, runtime: &RuntimePaths) -> io::Result<()> {
        if runtime != &self.request.runtime {
            return Err(invalid("Integrated runtime differs from its native storage admission"));
        }
        Ok(())
    }
}
impl PreparedInstallation {
    /// Admit fresh setup integration on an execution worker before sending its versioned payload.
    /// Managed process mode alone grants no data ownership: the unchanged standalone marker,
    /// Ready attempt and native source association must pass the same runtime storage verifier.
    /// Attached storage retains its selected normal base. Keep the returned owner until the
    /// actual worker command settles; it grants no application run or generation activation.
    /// # Errors
    /// Rejects missing/contradictory provenance, foreign attachment, links, uncertain ownership,
    /// ended controls and admission exhaustion before any integration command can have effects.
    pub fn integration(
        &self,
        runtime: RuntimePaths,
        context: &JobContext,
    ) -> io::Result<IntegrationStorage> {
        if self.session.existing.is_some() {
            return Err(invalid("Maintenance preserves its existing integrated configuration"));
        }
        admit(&self.session.admission.root, &self.session.selection, runtime, context)
    }
}
fn admit(
    root: &Directory,
    selection: &Selection,
    runtime: RuntimePaths,
    context: &JobContext,
) -> io::Result<IntegrationStorage> {
    check(context)?;
    selection.validate()?;
    let memory = context.memory().claim(2 * 1024 * 1024).map_err(io::Error::other)?;
    let marker = super::super::ownership::read(root)?;
    let source = Directory::open(&runtime.directory, false)?;
    let mut inputs = vec![marker];
    let (storage_base, storage) = match selection.mode.as_str() {
        "attached" => {
            let selected = Directory::open(&selection.comfy_root, false)?;
            if selected.native_identity()? != source.native_identity()? {
                return Err(invalid(
                    "Attached integration source differs from the selected native directory",
                ));
            }
            (runtime.directory.clone(), None)
        }
        "managed" => {
            inputs.extend(super::super::backend_data::provisioning::verify(
                &selection.comfy_root,
                &source,
                context,
            )?);
            let base = sugarsubstitute_configuration::layout::InstallationLayout::new(root.path())
                .comfy_data();
            let storage = user_paths::create_parents(root, &base, context)?;
            (base, Some(storage))
        }
        _ => return Err(invalid("Unsupported integration storage provenance")),
    };
    check(context)?;
    Ok(IntegrationStorage {
        request: IntegrationRequest {
            version: 2,
            runtime,
            storage_base,
            models: selection.models.clone(),
        },
        _root: root.retained()?,
        _source: source,
        _storage: storage,
        _inputs: inputs,
        _memory: memory,
    })
}
#[cfg(test)]
mod tests;
