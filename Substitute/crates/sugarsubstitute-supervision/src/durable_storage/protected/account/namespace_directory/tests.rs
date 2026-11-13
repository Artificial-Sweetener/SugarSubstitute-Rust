// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake native name observation while exercising actual namespace child derivation.
use super::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;
/// Reject ambiguous feature names before any native account creation or open occurs.
#[test]
fn invalid_runtime_child_names_have_no_directory_effects() {
    for leaf in
        ["", "../outside", "nested/child", r"nested\child", "record:stream", "NUL", "child."]
    {
        let mut boundary = Observed { calls: Vec::new(), first: PathBuf::new(), fail_at: None };
        let cause = resolve_with_boundary(
            PathBuf::from(r"\\?\C:\Sugar"),
            Namespace::RuntimeChild(leaf),
            &mut boundary,
        );
        assert!(cause.is_err(), "Accepted invalid account child");
        assert!(boundary.calls.is_empty());
    }
}
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
    let logical = PathBuf::from(r"\\?\C:\Sugar");
    let physical = PathBuf::from(r"\\?\C:\Redirected\Sugar\runtime");
    let mut boundary = Observed { calls: Vec::new(), first: physical.clone(), fail_at: None };
    let directory = resolve_with_boundary(logical.clone(), Namespace::Claims, &mut boundary)?;
    assert_eq!(
        boundary.calls,
        [
            logical.join("runtime"),
            physical.join("claims"),
            physical.join(r"claims\coordinators"),
            physical.join(r"claims\backends")
        ]
    );
    assert_eq!(directory, physical.join("claims"));
    let record = directory.join(r"coordinators\incident.record");
    assert_eq!(
        record.with_extension("nonce.stage"),
        physical.join(r"claims\coordinators\incident.nonce.stage")
    );
    Ok(())
}
#[test]
fn unchanged_native_names_preserve_directory_order_and_final_claim_root() -> TestResult {
    let parent = PathBuf::from(r"\\?\D:\SugarSubstitute");
    let mut boundary = Observed { calls: Vec::new(), first: parent.join("runtime"), fail_at: None };
    assert_eq!(
        resolve_with_boundary(parent.clone(), Namespace::Claims, &mut boundary)?,
        parent.join(r"runtime\claims")
    );
    assert_eq!(boundary.calls.len(), 4);
    Ok(())
}
#[test]
fn failed_final_name_preserves_genuine_cause_and_stops_child_creation() -> TestResult {
    let mut boundary = Observed {
        calls: Vec::new(),
        first: PathBuf::from(r"\\?\D:\physical\runtime"),
        fail_at: Some(2),
    };
    let cause = match resolve_with_boundary(
        PathBuf::from(r"\\?\D:\logical"),
        Namespace::Claims,
        &mut boundary,
    ) {
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

/// Model native redirection at the newly created history leaf, after an unchanged existing parent.
#[test]
fn operational_history_uses_the_final_native_leaf_without_creating_claim_subdirectories()
-> TestResult {
    struct RedirectedHistory {
        calls: Vec<PathBuf>,
        physical: PathBuf,
    }
    impl DirectoryBoundary for RedirectedHistory {
        fn create_and_observe(&mut self, path: &Path) -> Result<PathBuf, ClaimError> {
            self.calls.push(path.to_owned());
            Ok(if self.calls.len() == 2 { self.physical.clone() } else { path.to_owned() })
        }
    }
    let logical = PathBuf::from(r"\\?\C:\Sugar");
    let physical = PathBuf::from(r"\\?\C:\Redirected\Sugar\runtime\history");
    let mut boundary = RedirectedHistory { calls: Vec::new(), physical: physical.clone() };
    assert_eq!(
        resolve_with_boundary(logical.clone(), Namespace::RuntimeChild("history"), &mut boundary)?,
        physical
    );
    assert_eq!(boundary.calls, [logical.join("runtime"), logical.join(r"runtime\history")]);
    Ok(())
}
