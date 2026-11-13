// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recognize upstream startup landmarks for feedback, never for readiness authority.
use sugarsubstitute_contracts::startup::StartupOperation;

/// Classify a decoded ANSI-free transcript line without copying paths or node names
/// into owned product text. Unknown or changed upstream copy remains console output.
pub fn operation(line: &str) -> Option<StartupOperation> {
    let line = line.trim().strip_prefix("[INFO] ").unwrap_or(line.trim());
    if [
        "Collecting ",
        "Downloading ",
        "Installing collected packages:",
        "Install: pip packages for ",
        "Install: install script for ",
        "## Execute management script for ",
        "[ComfyUI-Manager] Starting dependency installation",
        "[ComfyUI-Manager] Restarting to reapply dependency installation.",
    ]
    .iter()
    .any(|prefix| line.starts_with(prefix))
    {
        Some(StartupOperation::Dependencies)
    } else if [
        "[DONE] Security scan",
        "Prestartup times for custom nodes:",
        "[ComfyUI-Manager] Startup script completed.",
        "Checkpoint files will always be loaded safely.",
    ]
    .iter()
    .any(|prefix| line.starts_with(prefix))
    {
        Some(StartupOperation::Runtime)
    } else if ["Total VRAM ", "pytorch version:", "Set vram state to:", "Device:"]
        .iter()
        .any(|prefix| line.starts_with(prefix))
        || (line.starts_with("Using ") && line.contains("attention"))
    {
        Some(StartupOperation::Device)
    } else if [
        "### Loading:",
        "Trying to load custom node ",
        "Importing custom node ",
        "[Prompt Server] web root:",
        "Import times for custom nodes:",
    ]
    .iter()
    .any(|prefix| line.starts_with(prefix))
    {
        Some(StartupOperation::CustomNodes)
    } else if ["Context impl SQLiteImpl.", "Running upgrade ", "Database upgraded from "]
        .iter()
        .any(|prefix| line.starts_with(prefix))
    {
        Some(StartupOperation::Database)
    } else if line == "Starting server" || line.starts_with("To see the GUI go to:") {
        Some(StartupOperation::Server)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upstream_markers_describe_work_without_claiming_readiness() {
        for (line, expected) in [
            ("Collecting torch", StartupOperation::Dependencies),
            ("[INFO] Downloading torch-2.whl", StartupOperation::Dependencies),
            ("[DONE] Security scan", StartupOperation::Runtime),
            ("Device: cuda:0 NVIDIA", StartupOperation::Device),
            ("### Loading: Custom nodes", StartupOperation::CustomNodes),
            ("Context impl SQLiteImpl.", StartupOperation::Database),
            ("To see the GUI go to: http://127.0.0.1:8188", StartupOperation::Server),
        ] {
            assert_eq!(operation(line), Some(expected));
        }
        assert_eq!(operation("Some plugin says everything is ready"), None);
        assert_eq!(operation(""), None);
    }
}
