// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual copy-first imported pages remain visible with provenance and retain original bytes.
use super::*;
use crate::{
    durable_storage::{imports, user_paths},
    setup_storage::test_worker::{self as worker, TestResult},
};
use std::{io::Write, path::PathBuf};
use sugarsubstitute_contracts::installation::InstallationId;

struct Fixture {
    _temporary: tempfile::TempDir,
    root: Directory,
    base: PathBuf,
}
impl Fixture {
    fn new(context: &JobContext) -> io::Result<Self> {
        let temporary = worker::installation(0xd1, context).map_err(io::Error::other)?;
        let root = Directory::open(temporary.path(), false)?;
        let base = root.path().join("user");
        user_paths::create_parents(&root, &base, context)?;
        Ok(Self { _temporary: temporary, root, base })
    }
    fn preserve(
        &self,
        provenance: &str,
        event: &str,
        context: &JobContext,
    ) -> io::Result<(PathBuf, PathBuf)> {
        let source = self.root.path().join(format!("old-{provenance}"));
        protection::with(context, |owner| {
            owner.directory(&source, Mode::CreateNew, context)?;
            append_at(&source, &[event.into()], 0, owner, context)
        })?;
        let parent = self.base.join("runtime").join(provenance);
        user_paths::create_parents(&self.root, &parent, context)?;
        let target = parent.join(DIRECTORY);
        let original = Directory::open(&source, false)?;
        with_closed_source(&original, HistorySource::Original, context, |source, excluded| {
            imports::copy_tree(
                &self.root,
                imports::ImportAssociation::Installed(
                    &InstallationId::new(format!("{:032x}", 0xd1)).map_err(io::Error::other)?,
                ),
                source,
                &target,
                imports::RootEntries::Except(excluded),
                context,
            )
        })?;
        Ok((source, target))
    }
}
#[test]
fn export_includes_account_and_installation_copies_without_creating_passive_writers() -> TestResult
{
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let fixture = Fixture::new(context)?;
        let (account, account_copy) =
            fixture.preserve("account-history", "operation=earlier_account\n", context)?;
        let (installation, installation_copy) = fixture.preserve(
            "installation-history",
            "operation=earlier_installation\n",
            context,
        )?;
        let account_bytes = std::fs::read(account.join("page-0.json"))?;
        let installation_bytes = std::fs::read(installation.join("page-0.json"))?;
        append(&fixture.base, &["operation=current\n".into()], 0, context)?;
        let exported = export(&fixture.base, context)?;
        assert_eq!(
            std::str::from_utf8(exported.as_bytes())?,
            "operation=current\nhistory_source=preserved_account\noperation=earlier_account\nhistory_source=preserved_installation\noperation=earlier_installation\n"
        );
        for target in [&account_copy, &installation_copy] {
            assert!(!target.join("writer").exists());
        }
        assert_eq!(std::fs::read(account.join("page-0.json"))?, account_bytes);
        assert_eq!(std::fs::read(installation.join("page-0.json"))?, installation_bytes);
        Ok(())
    })
}
#[test]
fn passive_only_export_preserves_interrupted_stages_and_refuses_future_pages_without_repair()
-> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let fixture = Fixture::new(context)?;
        let (original, target) =
            fixture.preserve("account-history", "operation=preserved\n", context)?;
        let source_bytes = std::fs::read(original.join("page-0.json"))?;
        let stage = target.join("record-stream-0123456789abcdef0123456789abcdef");
        protection::with(context, |owner| {
            let mut file = owner.file(&stage, context)?;
            file.write_all(b"{interrupted")?;
            files::sync(&file, context)?;
            owner.close_file(file)
        })?;
        let exported = export(&fixture.base, context)?;
        assert_eq!(exported.as_bytes(), b"history_source=preserved_account\noperation=preserved\n");
        assert_eq!(std::fs::read(&stage)?, b"{interrupted");
        assert!(!target.join("writer").exists());
        let future = b"{\"schema\":2,\"sequence\":1,\"omitted\":0,\"events\":[]}";
        write(&Directory::open(&target, false)?, "page-0.json", future, context)?;
        assert!(export(&fixture.base, context).is_err());
        assert_eq!(std::fs::read(target.join("page-0.json"))?, future);
        assert_eq!(std::fs::read(&stage)?, b"{interrupted");
        assert_eq!(std::fs::read(original.join("page-0.json"))?, source_bytes);
        assert!(!target.join("writer").exists());
        Ok(())
    })
}
