// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The save dialog opens only at a folder that exists and belongs to the user.
use super::*;
use std::error::Error;

type TestResult = Result<(), Box<dyn Error>>;

/// A fresh folder tree: a real profile with Documents, an installation whose redirected home
/// holds its own Documents and Desktop, a ComfyUI installation with a workflows folder, and a
/// folder the user exported to before.
struct Tree {
    root: PathBuf,
}
impl Tree {
    fn new(name: &str) -> Result<Self, Box<dyn Error>> {
        let root =
            std::env::temp_dir().join(format!("export-folder-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for folder in [
            r"Users\person\Documents",
            r"Reports",
            r"SugarSubstitute\user\comfy-data\Documents",
            r"SugarSubstitute\user\comfy-data\Desktop",
            r"ComfyUI\comfy",
            r"ComfyUI\user\default\workflows",
        ] {
            std::fs::create_dir_all(root.join(folder))?;
        }
        std::fs::write(root.join(r"SugarSubstitute\SugarSubstitute.exe"), b"")?;
        std::fs::write(root.join(r"ComfyUI\main.py"), b"")?;
        Ok(Self { root })
    }
    fn at(&self, folder: &str) -> PathBuf {
        self.root.join(folder)
    }
    /// Candidates as a desktop child with its home redirected into the installation sees them.
    fn candidates(&self, remembered: Option<&str>) -> Candidates {
        Candidates {
            remembered: remembered.map(|folder| self.at(folder)),
            documents: vec![
                self.at(r"SugarSubstitute\user\comfy-data\Documents"),
                self.at(r"Users\person\Documents"),
            ],
            profile: Some(self.at(r"Users\person")),
            excluded: vec![self.at("SugarSubstitute"), self.at(r"SugarSubstitute\user\comfy-data")],
        }
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn same(chosen: Option<PathBuf>, wanted: &Path) -> bool {
    chosen.as_deref() == Some(wanted)
}

/// The last folder the user exported to is used while it exists and is the user's.
#[test]
fn the_last_export_folder_comes_first() -> TestResult {
    let tree = Tree::new("last")?;
    let chosen = tree.candidates(Some("Reports")).choose();
    assert!(same(chosen.clone(), &tree.at("Reports")), "{chosen:?}");
    Ok(())
}

/// Without a usable remembered folder the user's real Documents folder is used, never the
/// redirected Desktop or Documents inside the installation, the ComfyUI workflows folder or a
/// folder that no longer exists.
#[test]
fn otherwise_the_real_documents_folder_is_used() -> TestResult {
    let tree = Tree::new("documents")?;
    let documents = tree.at(r"Users\person\Documents");
    for remembered in [
        None,
        Some(r"SugarSubstitute\user\comfy-data\Desktop"),
        Some(r"ComfyUI\user\default\workflows"),
        Some(r"Reports\removed"),
    ] {
        let chosen = tree.candidates(remembered).choose();
        assert!(same(chosen.clone(), &documents), "{remembered:?} chose {chosen:?}");
    }
    Ok(())
}

/// Without a Documents folder of the user's own, the profile folder is used.
#[test]
fn the_profile_folder_is_the_last_resort() -> TestResult {
    let tree = Tree::new("profile")?;
    std::fs::remove_dir(tree.at(r"Users\person\Documents"))?;
    let chosen = tree.candidates(None).choose();
    assert!(same(chosen.clone(), &tree.at(r"Users\person")), "{chosen:?}");
    Ok(())
}

/// A shell folder value is expanded with the real profile only; a value naming any other
/// variable, which the redirected environment could answer, is not used.
#[test]
fn a_shell_folder_value_expands_only_the_real_profile() -> TestResult {
    let profile = std::path::Path::new(r"C:\Users\person");
    assert_eq!(
        expand_profile(r"%USERPROFILE%\Documents", profile),
        Some(PathBuf::from(r"C:\Users\person\Documents"))
    );
    assert_eq!(expand_profile(r"D:\Docs", profile), Some(PathBuf::from(r"D:\Docs")));
    assert_eq!(expand_profile(r"%OneDrive%\Documents", profile), None);
    Ok(())
}
