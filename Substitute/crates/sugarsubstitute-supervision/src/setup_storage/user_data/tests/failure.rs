// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A rejected startup document keeps its typed cause visible to diagnostics.
use super::*;
use crate::setup_storage::test_worker::{self, TestResult};
use sugarsubstitute_configuration::startup::StartupConfigurationError;

#[test]
fn an_invalid_startup_document_keeps_its_typed_schema_cause_in_the_failure_chain() -> TestResult {
    test_worker::run(|context, _| {
        let directory = tempfile::tempdir()?;
        let configuration = directory.path().join("configuration.json");
        std::fs::write(&configuration, b"{invalid-json")?;
        let folder = directory.path().to_owned();
        let paths = StartupPaths::new([
            configuration,
            folder.clone(),
            folder.clone(),
            folder.clone(),
            folder.clone(),
            folder.clone(),
        ])?;
        let failure = prepare_launch_data(
            directory.path(),
            ConfigurationSource::Explicit(paths),
            |_| Ok::<_, io::Error>(ExplicitBackendDirectories::default()),
            |_, _| Ok(HistoricalArchiveDisposition::PreservedOriginal),
            context,
        )
        .err()
        .ok_or("An invalid startup document was accepted")?;
        let mut cause = failure.source();
        let mut schema = false;
        while let Some(current) = cause {
            schema |= current.is::<StartupConfigurationError>();
            cause = current.source();
        }
        assert!(schema, "The typed schema failure was replaced by its io::Error wrapper");
        Ok(())
    })
}
