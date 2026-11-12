// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove fatal causes stay readable link by link while sensitive links are withheld.
use super::*;

#[derive(Debug)]
struct Link(&'static str, Option<Box<dyn Error>>);
impl fmt::Display for Link {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl Error for Link {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.1.as_deref()
    }
}
fn chain(links: &[&'static str]) -> Box<dyn Error> {
    links
        .iter()
        .rev()
        .fold(None, |source, text| Some(Box::new(Link(text, source)) as Box<dyn Error>))
        .unwrap_or_else(|| Box::new(Link("empty", None)))
}

#[test]
fn every_non_sensitive_link_of_a_fatal_cause_stays_readable() {
    let cause = chain(&[
        "Production entry failed",
        "Authenticated entry preparation failed",
        "Lifecycle input rejected: Overloaded",
    ]);
    let text = CauseChain(cause.as_ref()).to_string();
    assert_eq!(
        text,
        "Production entry failed; caused by: Authenticated entry preparation failed; caused by: \
         Lifecycle input rejected: Overloaded"
    );
    assert!(!super::super::fields::sensitive("cause_chain", &text));
}

#[test]
fn a_link_naming_a_path_url_or_credential_is_withheld_alone() {
    let cause = chain(&[
        "Workspace load stopped",
        r"Could not open D:\Users\private\workspace.json",
        "Request to http://127.0.0.1:8188/api failed",
        "Bearer abc rejected",
        "Access is denied",
    ]);
    let text = CauseChain(cause.as_ref()).to_string();
    assert_eq!(
        text,
        "Workspace load stopped; caused by: [redacted]; caused by: [redacted]; caused by: \
         [redacted]; caused by: Access is denied"
    );
    assert!(!super::super::fields::sensitive("cause_chain", &text));
}

#[test]
fn startup_configuration_errors_end_the_chain_before_quoting_configuration() {
    let configuration = sugarsubstitute_configuration::startup::StartupConfigurationError::TooLarge;
    let shown = configuration.to_string();
    let outer = Link("Startup failed", Some(Box::new(configuration)));
    assert_eq!(CauseChain(&outer).to_string(), format!("Startup failed; caused by: {shown}"));
}

#[test]
fn deep_or_long_chains_stay_inside_the_routed_field_limit() {
    let deep = chain(&["link"; 20]);
    let text = CauseChain(deep.as_ref()).to_string();
    assert_eq!(text.matches("link").count(), LINKS);
    assert!(text.ends_with("; caused by: 4 deeper causes"), "{text}");
    let long: &'static str = Box::leak("x".repeat(4096).into_boxed_str());
    let text = CauseChain(chain(&["first", long]).as_ref()).to_string();
    assert!(text.len() <= BYTES, "{}", text.len());
    assert!(text.starts_with("first; caused by: xxx") && text.ends_with("..."));
}

#[cfg(all(windows, feature = "windows-native-host"))]
#[derive(Clone, Default)]
struct Routed(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
#[cfg(all(windows, feature = "windows-native-host"))]
impl std::io::Write for Routed {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("Fixture sink poisoned"))?
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(all(windows, feature = "windows-native-host"))]
#[test]
fn a_routed_fatal_event_shows_its_cause_where_a_failure_field_shows_nothing()
-> Result<(), Box<dyn Error>> {
    let cause = chain(&[
        "Production entry failed",
        r"Profile D:\Users\private could not open",
        "Lifecycle input rejected: Overloaded",
    ]);
    let sink = Routed::default();
    let writer = sink.clone();
    let subscriber = crate::diagnostics::fixture_subscriber(move || writer.clone());
    tracing::subscriber::with_default(subscriber, || {
        tracing::error!(
            operation = "production_ownership_failed",
            failure = %cause,
            cause_chain = %CauseChain(cause.as_ref()),
            "Production admission closed with its original retained cause"
        );
    });
    let bytes = sink.0.lock().map_err(|_| "Fixture sink poisoned")?;
    let line = std::str::from_utf8(&bytes)?;
    assert!(line.contains("failure=[redacted]"), "{line}");
    assert!(
        line.contains(
            "cause_chain=Production entry failed; caused by: [redacted]; caused by: Lifecycle \
             input rejected: Overloaded"
        ),
        "{line}"
    );
    assert!(!line.contains("private"), "{line}");
    Ok(())
}
