// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native-selected inert installation fixtures; no application, interpreter or process is run.
use super::*;
use crate::{
    durable_storage::{files::Directory, protected::directories::Mode, protection},
    setup_storage::{
        RuntimePaths, Selection, SelectionStore,
        test_worker::{self, TestResult},
    },
};
use std::path::PathBuf;
use sugarsubstitute_configuration::{descriptor::LaunchDescriptor, layout::InstallationLayout};
use sugarsubstitute_contracts::installation::{
    Generation, GenerationId, PackageDigest, PreparationIntent,
};
mod failure;
mod migration;
mod originals;

struct Fixture {
    _temporary: tempfile::TempDir,
    image: PathBuf,
}
impl Fixture {
    fn new(context: &JobContext) -> io::Result<Self> {
        let temporary = test_worker::installation(0xc1, context).map_err(io::Error::other)?;
        let root = Directory::open(temporary.path(), false)?;
        let generation = Generation {
            id: GenerationId::new("a".repeat(32)).map_err(io::Error::other)?,
            package: PackageDigest::new("b".repeat(64)).map_err(io::Error::other)?,
            release: sugarsubstitute_contracts::installation::SignedRelease::new(
                "1.0.0".to_owned(),
                1,
            )
            .map_err(io::Error::other)?,
        };
        let image = root.path().join("releases").join(generation.id.as_str());
        protection::with(context, |owner| {
            owner.directory(&root.path().join("releases"), Mode::CreateNew, context)?;
            owner.directory(&image, Mode::CreateNew, context)?;
            Ok(())
        })?;
        let layout = InstallationLayout::new(root.path());
        let runtime = RuntimePaths {
            directory: root.path().join("external-comfy"),
            python: root.path().join("python.exe"),
            frontend: root.path().join("frontend"),
            metadata: root.path().join("METADATA"),
        };
        let selection = Selection {
            locale: "en".into(),
            install_root: root.path().to_owned(),
            mode: "attached".into(),
            comfy_root: runtime.directory.clone(),
            python: String::new(),
            models: root.path().join("selected models"),
            outputs: root.path().join("selected outputs"),
            selection_token: None,
        };
        let startup = format!(
            "# 原本 café\r\n{}",
            sugarsubstitute_configuration::initial::external(
                &runtime.directory,
                &runtime.python,
                &selection.outputs,
                "en"
            )?
        );
        std::fs::write(image.join("startup.local.toml"), startup)?;
        let paths = StartupPaths::new([
            image.join("startup.local.toml"),
            runtime.frontend.clone(),
            runtime.metadata.clone(),
            root.path().join("SugarCubes"),
            runtime.directory.join("main.py"),
            layout.webview_profile(),
        ])
        .map_err(io::Error::other)?;
        std::fs::write(
            image.join("sugarsubstitute-launch.local.json"),
            LaunchDescriptor::encode(paths)?,
        )?;
        std::fs::write(image.join("installation-ready.json"), serde_json::to_vec(&serde_json::json!({ "schema_version":1,"bundle_sha256":generation.package.as_str(),"selection":selection,"runtime":runtime })).map_err(io::Error::other)?)?;
        let store = SelectionStore::open(root.path(), context).map_err(io::Error::other)?;
        let before = store
            .load(context)
            .map_err(io::Error::other)?
            .ok_or_else(|| crate::durable_storage::invalid("Fixture selection absent"))?;
        let staging = before
            .prepare(generation.clone(), PreparationIntent::Automatic)
            .map_err(io::Error::other)?;
        store.compare_exchange(&before, &staging, context).map_err(io::Error::other)?;
        let after = staging.prepared(&generation).map_err(io::Error::other)?;
        store.compare_exchange(&staging, &after, context).map_err(io::Error::other)?;
        Ok(Self { _temporary: temporary, image })
    }
    fn installation(&self, context: &JobContext) -> io::Result<InstallationData> {
        InstallationData::open(&self.image, context)?
            .ok_or_else(|| crate::durable_storage::invalid("Fixture installation absent"))
    }
    fn controls(&self, context: &JobContext) -> io::Result<Controls> {
        let mut controls = Controls::default();
        controls.resolve(
            ConfigurationSource::Installed(self.image.join("sugarsubstitute-launch.local.json")),
            context,
        )?;
        controls.ready = Some(controls::ControlFile::read(
            &self.image.join("installation-ready.json"),
            1024 * 1024,
            context,
        )?);
        Ok(controls)
    }
}
