// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove known filesystem acquisition routes and stale reviews fail across authored script roots.
use super::*;
use crate::fixtures::{self, Repository};

fn fixture(source: &str) -> Result<Repository> {
    let mut repository = Repository::new()?;
    repository.architecture.owners[0].auxiliary_roots.push("worker".into());
    fixtures::write(repository.root(), "worker/boundary.ts", source)?;
    Ok(repository)
}

#[test]
fn module_aliases_dynamic_loaders_escaped_names_and_computed_properties_are_rejected() -> Result<()>
{
    for source in [
        "import disk from 'node:fs'; disk.writeFileSync(path, bytes)",
        "import {open as receive} from 'fs/promises'",
        "export * from 'node:fs/promises'",
        "const {writeFile: publish} = require('fs')",
        "const module = await import('node:fs')",
        "process['getBuiltinModule'](name)",
        "const load = process.getBuiltinModule; load(name)",
        "process.binding('fs')",
        "require('original-fs')",
        "require('fs-extra')",
        r"require('node:\x66s')",
        r"require('\u0066s/promises')",
        r"require('\u{66}s')",
        r"process.getBuiltin\u004dodule(name)",
        r"process.\u{67}etBuiltinModule(name)",
        "import {download as transfer} from '@upstream/lib/download'",
        r"import {download as transfer} from '@upstream/lib\\download'",
        "export * from '../../third_party/comfy-desktop/src/main/lib/cache.ts'",
        "const {defaultCache: storage} = require('@upstream/lib/cache')",
        "const value = `${require('node:fs').readFileSync(path)}`",
    ] {
        let repository = fixture(source)?;
        assert!(
            check(repository.root(), &repository.architecture, &[]).is_err(),
            "accepted {source}"
        );
    }
    let repository =
        fixture("// import disk from 'node:fs'\nconst bytes = await nativeDownloads.read(name)")?;
    check(repository.root(), &repository.architecture, &[])
}

#[test]
fn reviewed_external_boundary_is_exact_owned_and_cannot_be_stale_or_unused() -> Result<()> {
    let repository = fixture(
        "import {access} from 'node:fs/promises'; export async function inspect(path) { await access(path) }",
    )?;
    let path = "worker/boundary.ts".to_owned();
    let mut boundary = Boundary {
        path: path.clone(),
        owner: "host".into(),
        rationale: "Existing external runtime inspection".into(),
        source_sha256: fingerprint(repository.root(), std::slice::from_ref(&path))?,
        role: Role::ExternalRuntime,
    };
    check(repository.root(), &repository.architecture, std::slice::from_ref(&boundary))?;
    boundary.owner = "foreign".into();
    assert!(
        check(repository.root(), &repository.architecture, std::slice::from_ref(&boundary))
            .is_err()
    );
    boundary.owner = "host".into();
    fixtures::write(repository.root(), &path, "import disk from 'node:fs'; disk.unlinkSync(path)")?;
    assert!(
        check(repository.root(), &repository.architecture, std::slice::from_ref(&boundary))
            .is_err()
    );
    fixtures::write(repository.root(), &path, "const value = nativeDownloads.length(name)")?;
    boundary.source_sha256 = fingerprint(repository.root(), std::slice::from_ref(&path))?;
    assert!(check(repository.root(), &repository.architecture, &[boundary]).is_err());
    Ok(())
}

#[test]
fn native_injection_runtime_and_fixture_roles_cannot_disguise_other_capabilities() -> Result<()> {
    for (source, role) in [
        ("import disk from 'node:fs'", Role::NativeInjection),
        ("import {download} from '@upstream/lib/download'", Role::ExternalRuntime),
        ("import {download} from '@upstream/lib/download'", Role::NativeInjection),
        ("import disk from 'node:fs'", Role::Fixture),
    ] {
        let repository = fixture(source)?;
        let path = "worker/boundary.ts".to_owned();
        let boundary = Boundary {
            source_sha256: fingerprint(repository.root(), std::slice::from_ref(&path))?,
            path,
            owner: "host".into(),
            rationale: "Deliberately invalid role".into(),
            role,
        };
        assert!(check(repository.root(), &repository.architecture, &[boundary]).is_err());
    }
    let repository = fixture(
        "import {download} from '@upstream/lib/download'; import {nativeDownloads} from './downloads'",
    )?;
    let path = "worker/boundary.ts".to_owned();
    let boundary = Boundary {
        source_sha256: fingerprint(repository.root(), std::slice::from_ref(&path))?,
        path,
        owner: "host".into(),
        rationale: "Native capability injection adapter".into(),
        role: Role::NativeInjection,
    };
    check(repository.root(), &repository.architecture, &[boundary])?;
    let repository = fixture("const storage = nativeDownloads()")?;
    let path = "worker/tests/actor.mjs".to_owned();
    fixtures::write(repository.root(), &path, "import disk from 'node:fs'")?;
    let boundary = Boundary {
        source_sha256: fingerprint(repository.root(), std::slice::from_ref(&path))?,
        path,
        owner: "host".into(),
        rationale: "Explicit owned fixture actor".into(),
        role: Role::Fixture,
    };
    check(repository.root(), &repository.architecture, &[boundary])
}

#[test]
fn reviewed_http_fixture_does_not_authorize_adjacent_runtime_filesystem_access() -> Result<()> {
    let repository = fixture("const storage = nativeDownloads()")?;
    let path = "worker/tests/http-scenarios.mjs".to_owned();
    fixtures::write(repository.root(), &path, "import disk from 'node:fs/promises'")?;
    let boundary = Boundary {
        source_sha256: fingerprint(repository.root(), std::slice::from_ref(&path))?,
        path,
        owner: "host".into(),
        rationale: "Owned HTTP qualification destinations".into(),
        role: Role::Fixture,
    };
    check(repository.root(), &repository.architecture, std::slice::from_ref(&boundary))?;
    fixtures::write(repository.root(), "worker/http.ts", "import disk from 'node:fs/promises'")?;
    assert!(check(repository.root(), &repository.architecture, &[boundary]).is_err());
    Ok(())
}

#[test]
fn tests_and_html_surfaces_have_the_same_filesystem_boundary_rule() -> Result<()> {
    for name in [
        "boundary.test.mjs",
        "preload.cjs",
        "surface.html",
        "inactive-platform.ts",
        "SURFACE.HTML",
        "PRELOAD.CJS",
    ] {
        let mut repository = Repository::new()?;
        repository.architecture.owners[0].auxiliary_roots.push("worker".into());
        fixtures::write(repository.root(), &format!("worker/{name}"), "require('node:fs')")?;
        assert!(check(repository.root(), &repository.architecture, &[]).is_err());
    }
    Ok(())
}
