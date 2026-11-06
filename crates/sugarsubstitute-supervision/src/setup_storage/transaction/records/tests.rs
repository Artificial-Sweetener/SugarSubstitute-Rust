// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reopen published Unicode startup documents through their authoritative configuration owner.
use super::*;
use crate::setup_storage::test_worker::{self, TestResult};
use sugarsubstitute_configuration::{
    descriptor::LaunchDescriptor, startup::parse_startup_configuration,
};
use sugarsubstitute_contracts::configuration::{ConnectionConfiguration, Theme};

#[test]
fn native_publication_reopens_with_the_runtime_configuration_contract() -> TestResult {
    test_worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let generation = temporary.path().join("generation 糖");
        let retained = Directory::open(&generation, true)?;
        let runtime = RuntimePaths {
            directory: temporary.path().join("ComfyUI café"),
            python: temporary.path().join("Python/python.exe"),
            frontend: temporary.path().join("frontend"),
            metadata: temporary.path().join("METADATA"),
        };
        let selection = Selection {
            locale: "ja".into(),
            install_root: temporary.path().join("installed"),
            mode: "attached".into(),
            comfy_root: runtime.directory.clone(),
            python: String::new(),
            models: temporary.path().join("models"),
            outputs: temporary.path().join("出力 🙂"),
            selection_token: None,
        };
        let cube = temporary.path().join("SugarCubes");
        write(retained.path(), &selection, &runtime, &cube)?;
        check(context)?;
        drop(retained);
        let restarted = Directory::open(&generation, false)?;
        let mut file = restarted.read("startup.local.toml")?;
        let bytes = files::read_bounded(&mut file, 65536)?;
        let startup = parse_startup_configuration(&bytes)?;
        assert_eq!(startup.profile().as_str(), "default");
        assert_eq!(startup.presentation().locale().as_str(), "ja");
        assert_eq!(startup.presentation().theme(), Theme::Dark);
        let ConnectionConfiguration::Launch(launch) = startup.connection() else {
            return Err("Expected managed backend launch".into());
        };
        assert_eq!(launch.root(), runtime.directory);
        assert_eq!(launch.port().get(), 8188);
        assert_eq!(
            launch.arguments(),
            ["--output-directory", selection.outputs.to_str().ok_or("Unicode output path")?]
        );
        let mut descriptor = restarted.read("sugarsubstitute-launch.local.json")?;
        let paths = LaunchDescriptor::parse(&files::read_bounded(&mut descriptor, 65536)?)?
            .into_startup_paths()?
            .into_paths();
        assert_eq!(
            paths,
            [
                generation.join("startup.local.toml"),
                runtime.frontend,
                runtime.metadata,
                cube,
                runtime.directory.join("main.py"),
                selection.install_root.join("user/desktop-data/profile")
            ]
        );
        Ok(())
    })
}

#[test]
fn owned_publication_confines_comfy_defaults_below_installation_user() -> TestResult {
    test_worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let generation = Directory::open(&temporary.path().join("generation"), true)?;
        let runtime = RuntimePaths {
            directory: temporary.path().join("managed/ComfyUI"),
            python: temporary.path().join("managed/python.exe"),
            frontend: temporary.path().join("frontend"),
            metadata: temporary.path().join("METADATA"),
        };
        let selection = Selection {
            locale: "en".into(),
            install_root: temporary.path().join("installed"),
            mode: "managed".into(),
            comfy_root: temporary.path().join("managed"),
            python: String::new(),
            models: temporary.path().join("models"),
            outputs: temporary.path().join("explicit output"),
            selection_token: Some("selected-plan".into()),
        };
        write(generation.path(), &selection, &runtime, &temporary.path().join("cube"))?;
        let mut file = generation.read("startup.local.toml")?;
        let startup = parse_startup_configuration(&files::read_bounded(&mut file, 65536)?)?;
        let ConnectionConfiguration::Launch(launch) = startup.connection() else {
            return Err("Expected owned backend launch".into());
        };
        let data = selection.install_root.join("user").join("comfy-data");
        assert_eq!(
            launch.arguments(),
            [
                "--base-directory",
                data.to_str().ok_or("Unicode data path")?,
                "--user-directory",
                data.join("user").to_str().ok_or("Unicode user path")?,
                "--temp-directory",
                data.join("temp").to_str().ok_or("Unicode temp path")?,
                "--output-directory",
                selection.outputs.to_str().ok_or("Unicode output path")?,
            ]
        );
        check(context)?;
        Ok(())
    })
}
