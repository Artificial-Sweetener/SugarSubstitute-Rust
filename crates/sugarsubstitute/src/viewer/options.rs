// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Parse explicit standalone demonstration inputs without filesystem or execution effects.
use std::{error::Error, ffi::OsString, io, path::PathBuf};
use sugarsubstitute_contracts::configuration::LocaleId;
use sugarsubstitute_image_ops::Extent;

pub(super) struct Options {
    pub(super) gpu: Option<sugarsubstitute_image_gpu::AdapterKey>,
    pub(super) capture: bool,
    pub(super) cache: PathBuf,
    pub(super) image: Option<PathBuf>,
    pub(super) locale: String,
    pub(super) extent: Extent,
    pub(super) proof: Option<ProofPaths>,
    pub(super) proof_window: bool,
}
/// Explicit caller-owned outputs for headless viewer qualification.
pub(super) struct ProofPaths {
    pub(super) image: PathBuf,
    pub(super) metrics: PathBuf,
}
impl Options {
    /// Require an explicit storage anchor so qualification never grows an ambient temporary cache.
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
        while let Some(flag) = arguments.next() {
            let value =
                arguments.next().ok_or_else(|| invalid("Viewer option requires a value"))?;
            match flag.to_str() {
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
        let cache = cache.filter(|path| path.is_absolute()).ok_or_else(|| invalid("Use canvas-viewer --cache <absolute folder> [--image <file>] [--locale <id>] [--width <pixels>] [--height <pixels>]"))?;
        let extent = Extent::new(width.unwrap_or(1280), height.unwrap_or(800))?;
        if extent.width() > 3840 || extent.height() > 2160 {
            return Err(invalid(
                "Viewer client dimensions exceed the standalone presentation budget",
            )
            .into());
        }
        let proof = match (output, metrics) {
            (None, None) => None,
            (Some(output), Some(metrics)) if image.is_some() && output.is_absolute() && metrics.is_absolute() && output != metrics => Some(ProofPaths { image: output, metrics }),
            _ => return Err(invalid("Headless viewer proof requires --image, distinct absolute --output and --metrics paths").into()),
        };
        if proof_window.is_some() && proof.is_none() {
            return Err(invalid("Hidden native qualification requires proof outputs").into());
        }
        Ok(Self {
            gpu: renderer.flatten(),
            capture: proof.is_some(),
            cache,
            image,
            locale: locale.unwrap_or_else(|| "en".to_owned()),
            extent,
            proof,
            proof_window: proof_window.unwrap_or(false),
        })
    }
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
    fn viewer_options_require_explicit_storage_and_preserve_selected_source_units()
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
        assert!(Options::parse([]).is_err());
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
}
