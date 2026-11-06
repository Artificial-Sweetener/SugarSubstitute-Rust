// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Parse standalone inputs and locate the portable viewer's cache without creating storage.
use crate::native_canvas::{
    config::{Config, InitialSource},
    proof::ProofPaths,
};
use std::{error::Error, ffi::OsString, io, path::PathBuf};
use sugarsubstitute_contracts::configuration::LocaleId;
use sugarsubstitute_image_ops::Extent;

pub(super) struct Options {
    pub(super) gpu: Option<sugarsubstitute_image_gpu::AdapterKey>,
    pub(super) capture: bool,
    pub(super) verify_native: bool,
    pub(super) cache: PathBuf,
    pub(super) image: Option<PathBuf>,
    pub(super) locale: String,
    pub(super) extent: Extent,
    pub(super) proof: Option<ProofPaths>,
    pub(super) proof_window: bool,
}
impl Options {
    /// Transfer runtime inputs after the standalone host has retained its proof/window options.
    /// Fund storage for canvases up to `maximum_extent` physical pixels. Standalone selections
    /// are local files without host output identities, so the viewer retains no documents.
    pub(super) fn into_config(self, maximum_extent: Extent) -> Config {
        let Self {
            gpu,
            capture,
            verify_native,
            cache,
            image,
            locale,
            extent,
            proof: _,
            proof_window: _,
        } = self;
        Config {
            gpu: gpu.map(crate::native_canvas::gpu_policy::GpuRequest::Required),
            capture,
            verify_native,
            cache,
            initial: image.map_or(InitialSource::PickFile, InitialSource::File),
            locale,
            extent,
            maximum_extent,
            retained_document_bytes: 0,
        }
    }

    /// Resolve ordinary portable launches beside the executable; qualification requires an
    /// explicit absolute cache anchor. Parsing never creates a directory or evaluates images.
    pub(super) fn parse(
        arguments: impl IntoIterator<Item = OsString>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut arguments = arguments.into_iter();
        let mut cache = None;
        let mut image = None;
        let mut locale = None;
        let mut width = None;
        let mut height = None;
        let mut output = None;
        let mut metrics = None;
        let mut proof_window = None;
        let mut renderer = None;
        let mut motion = None;
        let mut native_readback = None;
        while let Some(flag) = arguments.next() {
            let value =
                arguments.next().ok_or_else(|| invalid("Viewer option requires a value"))?;
            match flag.to_str() {
                Some("--motion-duration-ms") if motion.is_none() => {
                    let ms = number(value)?;
                    if !(1..=10000).contains(&ms) {
                        return Err(invalid(
                            "Proof motion duration must be between 1 and 10000 milliseconds",
                        )
                        .into());
                    }
                    motion = Some(std::time::Duration::from_millis(u64::from(ms)));
                }
                Some("--native-display-verification-readback") if native_readback.is_none() => {
                    native_readback = Some(match value.to_str() {
                        Some("on") => true,
                        Some("off") => false,
                        _ => {
                            return Err(invalid(
                                "Use --native-display-verification-readback on or off",
                            )
                            .into());
                        }
                    });
                }
                Some("--renderer") if renderer.is_none() => {
                    let value = value
                        .into_string()
                        .map_err(|_| invalid("Renderer selector must be Unicode"))?;
                    renderer = Some(if value == "cpu" { None } else { Some(value.parse()?) });
                }
                Some("--cache") if cache.is_none() => cache = Some(PathBuf::from(value)),
                Some("--image") if image.is_none() => image = Some(PathBuf::from(value)),
                Some("--locale") if locale.is_none() => {
                    let text = value
                        .into_string()
                        .map_err(|_| invalid("Viewer locale must be Unicode"))?;
                    LocaleId::new(&text)?;
                    locale = Some(text);
                }
                Some("--width") if width.is_none() => width = Some(number(value)?),
                Some("--height") if height.is_none() => height = Some(number(value)?),
                Some("--output") if output.is_none() => output = Some(PathBuf::from(value)),
                Some("--metrics") if metrics.is_none() => metrics = Some(PathBuf::from(value)),
                Some("--proof-window") if proof_window.is_none() && value == "hidden" => {
                    proof_window = Some(true);
                }
                _ => return Err(invalid("Unknown or duplicate viewer option").into()),
            }
        }
        if cache.is_none() && (output.is_some() || metrics.is_some()) {
            return Err(invalid(
                "Headless qualification requires an explicit absolute --cache folder",
            )
            .into());
        }
        let cache = cache_anchor(cache, std::env::current_exe)?;
        let extent = Extent::new(width.unwrap_or(1280), height.unwrap_or(800))?;
        if extent.width() > 3840 || extent.height() > 2160 {
            return Err(invalid(
                "Viewer client dimensions exceed the standalone presentation budget",
            )
            .into());
        }
        let proof = match (output, metrics) {
            (None, None) => None,
            (Some(output), Some(metrics)) if image.is_some() && output.is_absolute() && metrics.is_absolute() && output != metrics => Some(ProofPaths { image: output, metrics, motion_duration: motion.unwrap_or_default() }),
            _ => return Err(invalid("Headless viewer proof requires --image, distinct absolute --output and --metrics paths").into()),
        };
        if proof_window.is_some() && proof.is_none() {
            return Err(invalid("Hidden native qualification requires proof outputs").into());
        }
        if (motion.is_some() || native_readback.is_some()) && proof.is_none() {
            return Err(invalid(
                "Motion qualification and native readback selection require proof outputs",
            )
            .into());
        }
        let verify_native = proof.is_some() && native_readback.unwrap_or(true);
        Ok(Self {
            gpu: renderer.flatten(),
            capture: verify_native,
            verify_native,
            cache,
            image,
            locale: locale.unwrap_or_else(|| "en".to_owned()),
            extent,
            proof,
            proof_window: proof_window.unwrap_or(false),
        })
    }
}
/// Resolve only the executable location when no explicit cache anchor was supplied. The existing
/// cache owner performs every storage operation later on an admitted worker.
fn cache_anchor(
    selected: Option<PathBuf>,
    executable: impl FnOnce() -> io::Result<PathBuf>,
) -> io::Result<PathBuf> {
    let path = match selected {
        Some(path) => path,
        None => {
            let executable = executable()?;
            let directory = executable
                .parent()
                .ok_or_else(|| invalid("Viewer executable has no parent directory"))?;
            sugarsubstitute_configuration::layout::InstallationLayout::for_image_directory(
                directory,
            )
            .user()
            .join("cache")
        }
    };
    if !path.is_absolute() {
        return Err(invalid("Viewer cache folder must be absolute"));
    }
    Ok(path)
}
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
fn number(value: OsString) -> Result<u32, Box<dyn Error>> {
    Ok(value.to_str().ok_or_else(|| invalid("Viewer dimension must be an integer"))?.parse()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn viewer_options_resolve_portable_storage_and_preserve_selected_source_units()
    -> Result<(), Box<dyn Error>> {
        let cache = if cfg!(windows) { "E:\\viewer-owned-cache" } else { "/viewer-owned-cache" };
        let selected = OsString::from("画像🙂.png");
        let options = Options::parse([
            "--cache".into(),
            cache.into(),
            "--image".into(),
            selected.clone(),
            "--locale".into(),
            "ja".into(),
        ])?;
        assert_eq!(options.image, Some(PathBuf::from(selected)));
        assert_eq!(options.locale, "ja");
        assert!(Options::parse([])?.cache.is_absolute());
        assert!(Options::parse(["--cache".into(), "relative".into()]).is_err());
        assert!(
            Options::parse(["--cache".into(), cache.into(), "--width".into(), "0".into()]).is_err()
        );
        assert!(
            Options::parse(["--cache".into(), cache.into(), "--locale".into(), "unknown".into()])
                .is_err()
        );
        Ok(())
    }
    #[test]
    fn portable_cache_uses_executable_location_and_explicit_anchor_avoids_discovery()
    -> Result<(), Box<dyn Error>> {
        let root =
            PathBuf::from(if cfg!(windows) { "E:\\portable-画像" } else { "/portable-画像" });
        assert_eq!(
            cache_anchor(None, || Ok(root.join("canvas-viewer.exe")))?,
            root.join("user/cache")
        );
        assert_eq!(
            cache_anchor(None, || Ok(root.join("releases/generation/canvas-viewer.exe")))?,
            root.join("user/cache"),
        );
        let explicit = root.join("selected-cache");
        assert_eq!(
            cache_anchor(Some(explicit.clone()), || Err(io::Error::other("not queried")))?,
            explicit
        );
        assert!(cache_anchor(None, || Err(io::Error::other("executable lookup failed"))).is_err());
        assert!(cache_anchor(Some(PathBuf::from("relative")), || Ok(root.join("viewer"))).is_err());
        Ok(())
    }
    #[test]
    fn hidden_native_qualification_requires_complete_explicit_artifact_inputs()
    -> Result<(), Box<dyn Error>> {
        let root = if cfg!(windows) { "E:\\viewer-proof" } else { "/viewer-proof" };
        let root = PathBuf::from(root);
        let mut arguments = vec![
            "--cache".into(),
            root.join("cache").into_os_string(),
            "--proof-window".into(),
            "hidden".into(),
        ];
        assert!(Options::parse(arguments.clone()).is_err());
        arguments.extend([
            "--image".into(),
            root.join("source.png").into_os_string(),
            "--output".into(),
            root.join("viewport.png").into_os_string(),
            "--metrics".into(),
            root.join("metrics.json").into_os_string(),
        ]);
        let options = Options::parse(arguments.clone())?;
        assert!(options.proof_window);
        assert!(options.proof.is_some());
        arguments[3] = "visible".into();
        assert!(Options::parse(arguments).is_err());
        Ok(())
    }
    #[test]
    fn renderer_selection_is_explicit_native_and_restart_scoped() -> Result<(), Box<dyn Error>> {
        let root = if cfg!(windows) { "E:\\viewer-owned-cache" } else { "/viewer-owned-cache" };
        let options = Options::parse([
            "--cache".into(),
            root.into(),
            "--renderer".into(),
            "vulkan:8086:a780".into(),
        ])?;
        assert_eq!(options.gpu.map(|key| key.to_string()).as_deref(), Some("vulkan:8086:a780"));
        assert!(!options.capture);
        assert!(
            Options::parse(["--cache".into(), root.into(), "--renderer".into(), "webgpu".into()])
                .is_err()
        );
        assert!(
            Options::parse([
                "--cache".into(),
                root.into(),
                "--renderer".into(),
                "cpu".into(),
                "--renderer".into(),
                "vulkan:8086:a780".into()
            ])
            .is_err()
        );
        let cpu =
            Options::parse(["--cache".into(), root.into(), "--renderer".into(), "cpu".into()])?;
        assert!(cpu.gpu.is_none());
        Ok(())
    }
    #[test]
    fn motion_and_readback_controls_are_bounded_to_complete_qualification_inputs()
    -> Result<(), Box<dyn Error>> {
        let root = if cfg!(windows) { "E:\\viewer-proof" } else { "/viewer-proof" };
        let root = PathBuf::from(root);
        let mut arguments = vec!["--cache".into(), root.join("cache").into_os_string()];
        for flags in
            [["--motion-duration-ms", "1000"], ["--native-display-verification-readback", "off"]]
        {
            let mut incomplete = arguments.clone();
            incomplete.extend(flags.map(OsString::from));
            assert!(Options::parse(incomplete).is_err());
        }
        arguments.extend([
            "--image".into(),
            root.join("source.png").into_os_string(),
            "--output".into(),
            root.join("viewport.png").into_os_string(),
            "--metrics".into(),
            root.join("metrics.json").into_os_string(),
        ]);
        for invalid in ["0", "10001", "-1", "invalid"] {
            let mut rejected = arguments.clone();
            rejected.extend(["--motion-duration-ms".into(), invalid.into()]);
            assert!(Options::parse(rejected).is_err());
        }
        let default = Options::parse(arguments.clone())?;
        assert!(default.capture);
        assert!(default.verify_native);
        assert!(default.proof.ok_or("Proof inputs absent")?.motion_duration.is_zero());
        arguments.extend([
            "--motion-duration-ms".into(),
            "1000".into(),
            "--native-display-verification-readback".into(),
            "off".into(),
        ]);
        let options = Options::parse(arguments.clone())?;
        assert!(!options.capture);
        assert!(!options.verify_native);
        assert_eq!(
            options.proof.ok_or("Proof inputs absent")?.motion_duration,
            std::time::Duration::from_secs(1)
        );
        arguments.extend(["--native-display-verification-readback".into(), "on".into()]);
        assert!(Options::parse(arguments).is_err());
        Ok(())
    }
}
