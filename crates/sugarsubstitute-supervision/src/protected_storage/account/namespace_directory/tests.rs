// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake native name observation while exercising actual namespace child derivation.
use super::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;
struct Observed {
    calls: Vec<PathBuf>,
    first: PathBuf,
    fail_at: Option<usize>,
}
impl DirectoryBoundary for Observed {
    fn create_and_observe(&mut self, path: &Path) -> Result<PathBuf, ClaimError> {
        self.calls.push(path.to_owned());
        if self.fail_at == Some(self.calls.len()) {
            return Err(ClaimError::new(
                "canonical_namespace_test",
                std::io::Error::from(std::io::ErrorKind::PermissionDenied),
            ));
        }
        Ok(if self.calls.len() == 1 { self.first.clone() } else { path.to_owned() })
    }
}
#[test]
fn physical_final_directory_drives_every_later_namespace_and_stage_name() -> TestResult {
    let logical = PathBuf::from(r"\\?\C:\account\Local");
    let physical = PathBuf::from(r"\\?\C:\account\Packages\Codex\LocalCache\Local\SugarSubstitute");
    let mut boundary = Observed { calls: Vec::new(), first: physical.clone(), fail_at: None };
    let directory = resolve_with_boundary(logical.clone(), &mut boundary)?;
    assert_eq!(
        boundary.calls,
        [
            logical.join("SugarSubstitute"),
            physical.join("runtime"),
            physical.join(r"runtime\claims"),
            physical.join(r"runtime\claims\coordinators"),
            physical.join(r"runtime\claims\backends")
        ]
    );
    assert_eq!(directory, physical.join(r"runtime\claims"));
    let record = directory.join(r"coordinators\incident.record");
    assert_eq!(
        record.with_extension("nonce.stage"),
        physical.join(r"runtime\claims\coordinators\incident.nonce.stage")
    );
    Ok(())
}
#[test]
fn unchanged_native_names_preserve_directory_order_and_final_claim_root() -> TestResult {
    let parent = PathBuf::from(r"\\?\D:\account");
    let mut boundary =
        Observed { calls: Vec::new(), first: parent.join("SugarSubstitute"), fail_at: None };
    assert_eq!(
        resolve_with_boundary(parent.clone(), &mut boundary)?,
        parent.join(r"SugarSubstitute\runtime\claims")
    );
    assert_eq!(boundary.calls.len(), 5);
    Ok(())
}
#[test]
fn failed_final_name_preserves_genuine_cause_and_stops_child_creation() -> TestResult {
    let mut boundary = Observed {
        calls: Vec::new(),
        first: PathBuf::from(r"\\?\D:\physical\SugarSubstitute"),
        fail_at: Some(2),
    };
    let cause = match resolve_with_boundary(PathBuf::from(r"\\?\D:\logical"), &mut boundary) {
        Err(cause) => cause,
        Ok(_) => return Err("Failed final observation accepted".into()),
    };
    assert_eq!(boundary.calls.len(), 2);
    let leaf = std::error::Error::source(&cause)
        .and_then(|source| source.downcast_ref::<std::io::Error>())
        .ok_or("Native cause lost")?;
    assert_eq!(leaf.kind(), std::io::ErrorKind::PermissionDenied);
    Ok(())
}
