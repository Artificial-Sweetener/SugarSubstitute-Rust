// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fresh setup retries retain the recorded candidate and explicit provisioning choices.
use super::*;
use sugarsubstitute_contracts::installation::PreparationIntent;
mod installed;
mod maintenance;
mod reservation;
mod supersession;

fn choices(path: &Path) -> Selection {
    Selection {
        locale: "en".into(),
        install_root: path.join("native"),
        mode: "attached".into(),
        comfy_root: path.join("comfy"),
        python: String::new(),
        models: path.join("models"),
        outputs: path.join("outputs"),
        selection_token: None,
    }
}

fn runtime(path: &Path) -> Result<RuntimePaths, Failure> {
    let directory = path.join("comfy");
    let cube = directory.join("custom_nodes/SugarCubes");
    std::fs::create_dir_all(&cube)?;
    std::fs::write(
        cube.join("pyproject.toml"),
        b"[project]\nname = 'SugarCubes'\nversion = '1.0.0'\n",
    )?;
    let runtime = RuntimePaths {
        directory,
        python: path.join("python.exe"),
        frontend: path.join("frontend"),
        metadata: path.join("METADATA"),
    };
    std::fs::create_dir(&runtime.frontend)?;
    std::fs::write(&runtime.python, b"inert, never executed")?;
    std::fs::write(&runtime.metadata, b"fixture metadata")?;
    Ok(runtime)
}

fn import(path: &Path, context: &JobContext) -> Result<SignedInstallation, Failure> {
    let session = InstallationSession::create(choices(path), context)?;
    import_session(path, session, PreparationIntent::Automatic, context)
}

/// Reuse the exact signed package with independently acquired fresh or maintenance custody.
fn import_session(
    path: &Path,
    session: InstallationSession,
    intent: PreparationIntent,
    context: &JobContext,
) -> Result<SignedInstallation, Failure> {
    let (manifest, compressed) = test_package::distribution()?;
    import_distribution(path, session, intent, (manifest, compressed), context)
}

/// Keep archive authentication and custody identical for historical and explicitly capable fixtures.
fn import_distribution(
    path: &Path,
    session: InstallationSession,
    intent: PreparationIntent,
    (manifest, compressed): (Vec<u8>, Vec<u8>),
    context: &JobContext,
) -> Result<SignedInstallation, Failure> {
    let archive = test_package::seal(&manifest, &compressed);
    let mut signed = SignedPackage::new(&archive, 1)?;
    signed.request.intent = intent;
    let source = path.join("setup.exe");
    if !source.exists() {
        std::fs::write(&source, [b"inert prefix".as_slice(), &archive].concat())?;
    }
    Ok(session.import_installer(
        InstallerSource {
            repository: [92; 32],
            trusted_root: &signed.root,
            roots: &[],
            metadata: signed.metadata.metadata(),
            descriptor: &signed.descriptor,
            executable: &source,
        },
        signed.request,
        BundleSource::Extract(&path.join("workspace")),
        || NOW,
        context,
    )?)
}

#[test]
fn fresh_signed_installation_completes_without_accepting_health_or_executing_python() -> TestResult
{
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let runtime = runtime(temporary.path())?;
        let prepared = import(temporary.path(), context)?.prepare(|| NOW, context)?;
        assert!(prepared.maintenance_runtime().is_none());
        let candidate = prepared.complete(&runtime, || NOW, context)?;
        let state = SelectionStore::open(&temporary.path().join("native"), context)?
            .load(context)?
            .ok_or("No selection")?;
        assert!(state.accepted().is_none());
        assert!(matches!(state.candidate(), Some(Candidate::Prepared { .. })));
        let startup = std::fs::read_to_string(candidate.directory().join("startup.local.toml"))?;
        assert!(startup.contains("port = 8188"));
        assert_eq!(std::fs::read(runtime.python)?, b"inert, never executed");
        let transfer = candidate.transfer(|| NOW, context, &mut None)?;
        assert!(transfer.executable().is_file());
        assert!(!temporary.path().join("native/SugarSubstitute.exe").exists());
        Ok(())
    })
}

#[test]
fn interrupted_first_installation_resumes_its_recorded_generation() -> TestResult {
    for after_configuration in [false, true] {
        resume_first_installation(after_configuration)?;
    }
    Ok(())
}

fn resume_first_installation(after_configuration: bool) -> TestResult {
    let temporary = tempfile::tempdir()?;
    let runtime = runtime(temporary.path())?;
    let path = temporary.path().to_owned();
    let interrupted_runtime = runtime.clone();
    worker::run_reserved_expiring(SIGNED_INSTALLER_RESERVATION_BYTES, move |context, clock| {
        let prepared = import(&path, context)?.prepare(|| NOW, context)?;
        let record = path.join("native/state/selection/.substitute-selection.json");
        let installation = path.join("native");
        clock.observe_checks(move || {
            let Some(bytes) = std::fs::read(&record).ok() else { return false };
            let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                return false;
            };
            if value["candidate"]["phase"] != "staging" {
                return false;
            }
            if !after_configuration {
                return true;
            }
            value["candidate"]["generation"]["id"].as_str().is_some_and(|id| {
                installation.join("releases").join(id).join("startup.local.toml").exists()
            })
        })?;
        assert!(prepared.complete(&interrupted_runtime, || NOW, context).is_err());
        Ok(())
    })?;
    let path = temporary.path().to_owned();
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, move |context, _| {
        let store = SelectionStore::open(&path.join("native"), context)?;
        let before = store.load(context)?.ok_or("No interrupted state")?;
        assert!(matches!(before.candidate(), Some(Candidate::Staging { .. })));
        let generation = before.candidate().ok_or("No candidate")?.generation().clone();
        let prior_configuration = if after_configuration {
            Some(std::fs::read(
                path.join("native/releases")
                    .join(generation.id.as_str())
                    .join("startup.local.toml"),
            )?)
        } else {
            None
        };
        let unknown = path.join("native/releases/unknown-user-file");
        std::fs::write(&unknown, b"preserve")?;
        assert!(InstallationSession::create(choices(&path), context).is_err());
        assert_eq!(std::fs::read(&unknown)?, b"preserve");
        std::fs::remove_file(&unknown)?;
        let candidate = import(&path, context)?.prepare(|| NOW, context)?.complete(
            &runtime,
            || NOW,
            context,
        )?;
        assert_eq!(
            candidate.directory().file_name().and_then(|s| s.to_str()),
            Some(generation.id.as_str())
        );
        assert_eq!(std::fs::read_dir(path.join("native/releases"))?.count(), 1);
        if let Some(before) = prior_configuration {
            assert_eq!(std::fs::read(candidate.directory().join("startup.local.toml"))?, before);
        }
        assert!(store.load(context)?.ok_or("No state")?.accepted().is_none());
        Ok(())
    })
}
