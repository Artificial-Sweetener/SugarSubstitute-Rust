// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reuse the sealed preview and actual receiver operation owner for one fixed download campaign.

use super::capture::current_ticks;
use crate::{
    DesktopError, MainDownloadFixture, MainPreviewSnapshot,
    main_events::{Command, Events},
    windows_main_receivers::{DownloadWitness, NativeString, Receivers},
};
use std::{
    path::{Component, Path, PathBuf},
    rc::Rc,
};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::{HSTRING, Interface, PWSTR};

/// The original preview metadata reserves this bound for the destination and fixture name.
pub(super) const STORAGE_BYTES: usize = 4608;

/// An observer of production-owned downloads; it never cancels or settles those operations.
pub(super) struct Qualification {
    destination: PathBuf,
    fixture: MainDownloadFixture,
    witness: Rc<DownloadWitness>,
    evaluation: Option<Result<(), DesktopError>>,
    issued: bool,
}
impl Qualification {
    /// Derive only a fixed child of the already admitted per-run D browser profile.
    pub(super) fn new(
        profile: &Path,
        fixture: MainDownloadFixture,
        witness: Rc<DownloadWitness>,
    ) -> Result<Self, DesktopError> {
        let source = profile.to_str().ok_or_else(|| failure("profile encoding"))?;
        if source.len() > 1800
            || !source.as_bytes().starts_with(b"D:\\")
            || source[3..].contains(':')
            || !profile.is_absolute()
            || profile
                .components()
                .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        {
            return Err(failure("qualification profile must be an admitted D path"));
        }
        let destination = profile.join("k1-downloads");
        if destination.capacity() + fixture.allocated_bytes() > STORAGE_BYTES {
            return Err(failure("qualification metadata bound"));
        }
        Ok(Self { destination, fixture, witness, evaluation: None, issued: false })
    }
    /// Set the native profile destination before the fixed current-context script can save files.
    pub(super) fn issue(
        &mut self,
        snapshot: MainPreviewSnapshot,
        core: &ICoreWebView2,
        receivers: &Receivers,
        events: &Events,
    ) -> Result<(), DesktopError> {
        if self.issued || !events.preview_quiescent() {
            return Err(failure("qualification admission"));
        }
        let ticks = current_ticks(snapshot.source_deadline())?;
        let delta = i128::from(snapshot.source_deadline().ticks() - ticks);
        let frequency = i128::from(snapshot.source_deadline().frequency());
        let remaining = u32::try_from((delta * 1000 + frequency - 1) / frequency)
            .map_err(|cause| DesktopError::new("qualify_downloads", cause))?
            .min(i32::MAX as u32);
        let parameters =
            parameters(snapshot.context(), snapshot.origin(), &self.fixture, remaining)?;
        // SAFETY: Exact current core/profile belong to this creating STA; destination is
        // derived from the admitted D profile, not frontend data or a general path command.
        unsafe {
            let core = core
                .cast::<ICoreWebView2_13>()
                .map_err(|cause| DesktopError::new("qualify_downloads", cause))?;
            let mut raw = std::ptr::null_mut();
            let status = (core.vtable().Profile)(core.as_raw(), &mut raw);
            // The getter transfers its reference, including any partial failed output.
            let profile =
                if raw.is_null() { None } else { Some(ICoreWebView2Profile::from_raw(raw)) };
            status.ok().map_err(|cause| DesktopError::new("qualify_downloads", cause))?;
            let profile = profile.ok_or_else(|| failure("native profile absent"))?;
            profile
                .SetDefaultDownloadFolderPath(&HSTRING::from(self.destination.as_os_str()))
                .map_err(|cause| DesktopError::new("qualify_downloads", cause))?;
            let mut actual = PWSTR::null();
            let status = profile.DefaultDownloadFolderPath(&mut actual);
            let actual = NativeString(actual);
            status.map_err(|cause| DesktopError::new("qualify_downloads", cause))?;
            if actual.0.is_null() {
                return Err(failure("native destination absent"));
            }
            let mut observed = String::with_capacity(4096);
            crate::main_events::copy_utf16(&mut observed, |index| {
                actual.0.as_ptr().add(index).read()
            })?;
            if Path::new(&observed) != self.destination {
                return Err(failure("native destination changed"));
            }
        }
        current_ticks(snapshot.source_deadline())?;
        if !events.preview_quiescent() {
            return Err(failure("qualification source changed"));
        }
        self.issued = true;
        receivers.call(Command::Downloads, "Runtime.evaluate", &parameters)?;
        crate::qualification_trace::progress(
            crate::qualification_trace::Progress::DownloadsPrepared,
            snapshot.identity(),
        );
        Ok(())
    }
    /// Interpret only the bounded completion of this exact owner-submitted fixed script.
    pub(super) fn accept(&mut self, payload: &str) -> Result<(), DesktopError> {
        if !self.issued || self.evaluation.is_some() {
            return Err(failure("duplicate qualification completion"));
        }
        self.evaluation = Some(decode(payload, &self.fixture));
        Ok(())
    }
    /// Native production operations, rather than the script receipt, prove download completion.
    pub(super) fn completed(
        &mut self,
        origin: sugarsubstitute_contracts::configuration::LoopbackOrigin,
    ) -> Result<bool, DesktopError> {
        match self.evaluation.take() {
            None => Ok(false),
            Some(Err(cause)) => Err(cause),
            Some(Ok(())) => {
                self.evaluation = Some(Ok(()));
                self.witness.completed(&self.destination, origin)
            }
        }
    }
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Proof {
    schema: u8,
    requests: u8,
    png_bytes: u64,
    png_sha256: String,
}

/// Keep original native causes separate while retaining bounded closed qualification diagnostics.
fn failure(reason: &'static str) -> DesktopError {
    DesktopError::message("qualify_downloads", reason)
}

/// Decode closed evidence without changing product health or granting native authority.
fn decode(payload: &str, fixture: &MainDownloadFixture) -> Result<(), DesktopError> {
    if payload.len() > crate::main_events::PAYLOAD_BYTES {
        return Err(failure("qualification result bound"));
    }
    let parsed: serde_json::Value = serde_json::from_str(payload)
        .map_err(|cause| DesktopError::new("qualify_downloads", cause))?;
    if parsed.get("exceptionDetails").is_some() {
        return Err(failure("installed export failed"));
    }
    if parsed.get("result").and_then(|value| value.get("type")).and_then(serde_json::Value::as_str)
        != Some("string")
    {
        return Err(failure("qualification result type"));
    }
    let value = parsed
        .get("result")
        .and_then(|value| value.get("value"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| failure("qualification result"))?;
    if value.len() > 512 {
        return Err(failure("qualification proof bound"));
    }
    let proof: Proof = serde_json::from_str(value)
        .map_err(|cause| DesktopError::new("qualify_downloads", cause))?;
    if proof.schema != 1
        || proof.requests != 3
        || proof.png_bytes != fixture.bytes()
        || proof.png_sha256 != fixture.digest_hex()
    {
        return Err(failure("qualification result identity"));
    }
    Ok(())
}

/// Encode only fixed code and inert pre-inspected data for the sealed top execution context.
fn parameters(
    context: i64,
    origin: sugarsubstitute_contracts::configuration::LoopbackOrigin,
    fixture: &MainDownloadFixture,
    remaining: u32,
) -> Result<String, DesktopError> {
    let origin = url::Url::parse(&crate::main_navigation::root_url(origin))
        .map_err(|cause| DesktopError::new("qualify_downloads", cause))?
        .origin()
        .ascii_serialization();
    let input = serde_json::json!({"name":fixture.name(),"bytes":fixture.bytes(),
        "sha256":fixture.digest_hex(),"remaining_ms":remaining,
            "origin":origin});
    let expression =
        format!("(async()=>{{const input={input};\n{}\n}})()", include_str!("downloads.js"));
    let parameters = serde_json::json!({"expression":expression,"contextId":context,
        "awaitPromise":true,"returnByValue":true})
    .to_string();
    if parameters.len() > 8192 {
        return Err(failure("qualification evaluation bound"));
    }
    Ok(parameters)
}

#[cfg(test)]
mod tests {
    use super::*;
    type TestResult = Result<(), Box<dyn std::error::Error>>;
    fn fixture() -> Result<MainDownloadFixture, DesktopError> {
        MainDownloadFixture::new("0.png".into(), 107745, &"5d".repeat(32))
    }
    fn report(schema: u8, requests: u8, bytes: u64, hash: &str) -> String {
        serde_json::json!({"result":{"type":"string","value":serde_json::json!({
            "schema":schema,"requests":requests,"png_bytes":bytes,"png_sha256":hash}).to_string()}})
        .to_string()
    }
    #[test]
    fn actual_fixed_script_and_maximum_fixture_fit_original_native_command_storage() -> TestResult {
        let fixture = MainDownloadFixture::new(
            format!("{}.png", "a".repeat(124)),
            2 * 1024 * 1024,
            &"ff".repeat(32),
        )?;
        let origin = sugarsubstitute_contracts::configuration::LoopbackOrigin::parse(
            "http://[::1]:65535/",
            65535,
        )?;
        let value = parameters(i64::MAX, origin, &fixture, i32::MAX as u32)?;
        assert!(value.len() <= 8192);
        let decoded: serde_json::Value = serde_json::from_str(&value)?;
        assert_eq!(decoded["contextId"], i64::MAX);
        assert_eq!(decoded["awaitPromise"], true);
        assert!(
            decoded["expression"]
                .as_str()
                .ok_or("Expression absent")?
                .contains("Comfy.ExportWorkflow")
        );
        Ok(())
    }
    #[test]
    fn script_proof_is_bound_to_exact_request_count_png_size_hash_and_schema() -> TestResult {
        let fixture = fixture()?;
        assert!(decode(&report(1, 3, 107745, &fixture.digest_hex()), &fixture).is_ok());
        for payload in [
            report(2, 3, 107745, &fixture.digest_hex()),
            report(1, 2, 107745, &fixture.digest_hex()),
            report(1, 3, 107744, &fixture.digest_hex()),
            report(1, 3, 107745, &"ff".repeat(32)),
        ] {
            assert!(decode(&payload, &fixture).is_err());
        }
        Ok(())
    }
    #[test]
    fn script_exceptions_wrong_native_result_type_unknown_fields_and_oversize_proofs_fail()
    -> TestResult {
        let fixture = fixture()?;
        let valid = report(1, 3, 107745, &fixture.digest_hex());
        let mut value: serde_json::Value = serde_json::from_str(&valid)?;
        value["exceptionDetails"] = serde_json::json!({"text":"private detail"});
        assert!(decode(&value.to_string(), &fixture).is_err());
        value = serde_json::from_str(&valid)?;
        value["result"]["type"] = "object".into();
        assert!(decode(&value.to_string(), &fixture).is_err());
        value["result"]["type"] = "string".into();
        value["result"]["value"] = "x".repeat(513).into();
        assert!(decode(&value.to_string(), &fixture).is_err());
        let unknown =
            valid.replace("png_bytes\\\":107745", "extra\\\":true,\\\"png_bytes\\\":107745");
        assert_ne!(unknown, valid);
        assert!(decode(&unknown, &fixture).is_err());
        Ok(())
    }
}
