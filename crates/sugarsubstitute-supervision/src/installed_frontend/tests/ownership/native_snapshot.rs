// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual strict inspection and workspace writes through the existing execution owner.
use super::*;
use crate::installed_frontend::native::SystemPort;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
use sugarsubstitute_contracts::execution::{JobOutcome, TerminalKind};
type NativeError = Box<dyn std::error::Error + Send + Sync>;
#[derive(Debug)]
struct FilesystemFailure {
    operation: &'static str,
    relative: &'static str,
    cause: std::io::Error,
}
impl std::fmt::Display for FilesystemFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} for {}: {}", self.operation, self.relative, self.cause)
    }
}
impl std::error::Error for FilesystemFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}
fn filesystem<T>(
    operation: &'static str,
    relative: &'static str,
    result: std::io::Result<T>,
) -> Result<T, NativeError> {
    result
        .map_err(|cause| Box::new(FilesystemFailure { operation, relative, cause }) as NativeError)
}
const CUBE_BYTES: &[u8] = b"export const original = true;\n";
struct Workspace {
    container: PathBuf,
    root: PathBuf,
    input: FrontendInstallationInput,
}
fn create_workspace() -> Result<Workspace, NativeError> {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let id = NEXT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .map_err(|_| "Workspace fixture identity exhausted")?;
    let container =
        std::env::temp_dir().join(format!("installed-snapshot-{}-{id}", std::process::id()));
    filesystem("create_container", "container", fs::create_dir(&container))?;
    let root = container.join("workspace");
    filesystem("create_directory", ".", fs::create_dir(&root))?;
    let package = root.join("package");
    let cube = root.join("SugarCubes");
    filesystem("create_directories", "package/static", fs::create_dir_all(package.join("static")))?;
    filesystem("create_directories", "SugarCubes/web", fs::create_dir_all(cube.join("web")))?;
    let metadata = root.join("METADATA");
    let script = root.join("main.py");
    filesystem(
        "create_file",
        "METADATA",
        fs::write(&metadata, b"Name: comfyui_frontend_package\nVersion: 1.47.11\n\n"),
    )?;
    filesystem("create_file", "main.py", fs::write(&script, b"print('owned backend fixture')\n"))?;
    filesystem(
        "create_file",
        "package/static/index.html",
        fs::write(package.join("static/index.html"), b"<html>owned frontend fixture</html>\n"),
    )?;
    filesystem(
        "create_file",
        "SugarCubes/pyproject.toml",
        fs::write(
            cube.join("pyproject.toml"),
            b"[project]\nname = 'SugarCubes'\nversion = '0.15.3'\n",
        ),
    )?;
    filesystem(
        "create_file",
        "SugarCubes/__init__.py",
        fs::write(cube.join("__init__.py"), b"WEB_DIRECTORY = 'web'\n"),
    )?;
    filesystem(
        "create_file",
        "SugarCubes/web/main.js",
        fs::write(cube.join("web/main.js"), CUBE_BYTES),
    )?;
    let input = FrontendInstallationInput::new(package, metadata, cube, script)?;
    Ok(Workspace { container, root, input })
}
fn perform<T: Send + 'static>(
    harness: &Harness,
    id: u64,
    action: impl FnOnce() -> Result<T, NativeError> + Send + 'static,
) -> Result<T, Box<dyn std::error::Error>> {
    let mut job =
        harness
            .client
            .submit_blocking(&harness.scope, Harness::spec(id, 1000)?, move |_| action())?;
    harness.wake.until(&harness.client, || matches!(job.status().phase, JobPhase::Terminal(_)))?;
    match job.try_take_completion().ok_or("Missing actual filesystem result")?.outcome {
        JobOutcome::Success(value) => Ok(value),
        JobOutcome::Failure(cause) => Err(cause as Box<dyn std::error::Error>),
        _ => Err("Actual filesystem job did not complete successfully".into()),
    }
}
fn inspect_native(harness: &mut Harness) -> TestResult {
    harness.owner.submit_inspection(&harness.client, &harness.scope, Harness::spec(2, 1000)?)?;
    harness.wake.until(&harness.client, || {
        harness
            .owner
            .pending
            .as_ref()
            .is_some_and(|job| matches!(job.status().phase, JobPhase::Terminal(_)))
    })
}
fn close_native(
    harness: &Harness,
) -> Result<
    sugarsubstitute_contracts::execution::JobCompletion<(), FrontendSourceError>,
    Box<dyn std::error::Error>,
> {
    let mut job =
        harness.owner.submit_close(&harness.client, &harness.scope, Harness::spec(4, 2000)?)?;
    harness.wake.until(&harness.client, || matches!(job.status().phase, JobPhase::Terminal(_)))?;
    job.try_take_completion().ok_or_else(|| "Missing actual native source cleanup".into())
}
fn edit_and_replace(root: &Path) -> Result<(), NativeError> {
    for relative in [
        "main.py",
        "METADATA",
        "package/static/index.html",
        "SugarCubes/web/main.js",
        "SugarCubes/pyproject.toml",
        "SugarCubes/__init__.py",
    ] {
        let path = root.join(relative);
        let writable =
            filesystem("open_for_write", relative, fs::OpenOptions::new().write(true).open(&path))?;
        drop(writable);
        filesystem(
            "overwrite_file",
            relative,
            fs::write(&path, b"workspace source edited after inspection\n"),
        )?;
        let replacement = path.with_extension("replacement");
        filesystem(
            "write_replacement",
            relative,
            fs::write(&replacement, b"atomically replaced after inspection\n"),
        )?;
        filesystem("replace_file", relative, fs::rename(&replacement, &path))?;
    }
    // Both names are inside this fixture's owned container; other tests never pin that parent.
    // The original inspection pins both selected directories and their ancestor chain.
    let moved = root.with_extension("moved");
    filesystem("rename_workspace", ". -> moved", fs::rename(root, &moved))?;
    filesystem("restore_workspace", "moved -> .", fs::rename(&moved, root))?;
    Ok(())
}
#[test]
fn native_completed_snapshot_allows_source_writes_atomic_replacement_and_root_rename() -> TestResult
{
    let mut harness = Harness::new(false, false)?;
    let Workspace { container, root, input } = perform(&harness, 1, create_workspace)?;
    harness.owner = InstalledFrontendOwner::new(input);
    inspect_native(&mut harness)?;
    let lease = harness.owner.take_inspection()?.ok_or("Missing actual native snapshot")?;
    let expected: [u8; 32] = Sha256::digest(CUBE_BYTES).into();
    assert_eq!(
        lease
            .manifest()
            .assets()
            .iter()
            .find(|asset| asset.path() == "/extensions/SugarCubes/main.js")
            .ok_or("Missing inspected Cube asset")?
            .sha256(),
        expected
    );
    let released =
        crate::registry::lock(&harness.owner.shared.retained).as_ref().is_some_and(|state| {
            state.handles.is_empty() && state.search == 0 && state.manifest.is_some()
        });
    let editing = root.clone();
    let result = perform(&harness, 3, move || edit_and_replace(&editing));
    let live_after_edit = lease.validate_live();
    let observed = lease
        .manifest()
        .assets()
        .iter()
        .find(|asset| asset.path() == "/extensions/SugarCubes/main.js")
        .map(FrontendAsset::sha256);
    let cleanup = close_native(&harness)?;
    let retired = lease.validate_live().is_err();
    let cleared = crate::registry::lock(&harness.owner.shared.retained).is_none();
    let removed = perform(&harness, 5, move || {
        filesystem("remove_container", "container", fs::remove_dir_all(container))?;
        Ok(())
    });
    drop(harness.owner);
    drop(harness.executor);
    // Old lifetime pins fail the actual edit attempt only after checked owner cleanup and shutdown.
    assert!(matches!(cleanup.outcome, JobOutcome::Success(())));
    removed?;
    assert!(retired && cleared);
    result?;
    live_after_edit?;
    assert!(released);
    assert_eq!(observed, Some(expected));
    Ok(())
}
struct FailFirstRelease(AtomicBool);
impl SourcePort for FailFirstRelease {
    fn inspect(
        &self,
        input: &FrontendInstallationInput,
        state: &mut Inspection,
        context: &JobContext,
    ) -> Result<(), FrontendSourceError> {
        SystemPort.inspect(input, state, context)
    }
    fn close(
        &self,
        state: &mut Inspection,
        context: &JobContext,
    ) -> Result<(), FrontendSourceError> {
        if !self.0.swap(true, Ordering::AcqRel) {
            return Err(FrontendSourceError::new(
                "fixture_inspection_release",
                std::io::Error::other("original inspection release failure"),
            ));
        }
        SystemPort.close(state, context)
    }
}
#[test]
fn native_failed_inspection_release_retains_real_handles_and_cause_until_checked_cleanup()
-> TestResult {
    use std::error::Error;
    let mut harness = Harness::new(false, false)?;
    let Workspace { container, root, input } = perform(&harness, 1, create_workspace)?;
    harness.owner = InstalledFrontendOwner::new(input);
    harness.owner.port = Arc::new(FailFirstRelease(AtomicBool::new(false)));
    inspect_native(&mut harness)?;
    let cause =
        harness.owner.take_inspection().err().ok_or("Failed native release published a lease")?;
    assert_eq!(cause.stage(), "fixture_inspection_release");
    assert_eq!(
        cause.source().ok_or("Original release cause missing")?.to_string(),
        "original inspection release failure"
    );
    assert!(harness.owner.take_inspection()?.is_none());
    let retained = crate::registry::lock(&harness.owner.shared.retained);
    assert!(retained.as_ref().is_some_and(|state| state.manifest.is_some()
        && state.handles.iter().any(crate::native_claims::files::Handle::is_armed)));
    drop(retained);
    let locked_script = root.join("main.py");
    assert!(perform(&harness, 3, move || Ok(fs::OpenOptions::new()
        .write(true)
        .open(locked_script)
        .is_err()))?);
    let cleanup = close_native(&harness)?;
    assert_eq!(cleanup.outcome.kind(), TerminalKind::Success);
    assert!(crate::registry::lock(&harness.owner.shared.retained).is_none());
    let editing = root.clone();
    let result = perform(&harness, 5, move || edit_and_replace(&editing));
    perform(&harness, 6, move || {
        filesystem("remove_container", "container", fs::remove_dir_all(container))?;
        Ok(())
    })?;
    result?;
    assert_eq!(cause.stage(), "fixture_inspection_release");
    Ok(())
}
