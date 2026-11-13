// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { after, before, describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';
import { COMPONENTS, releaseOptions } from '../lib/components.mjs';
import { requireRecordedDigests, target } from '../lib/github-release.mjs';
import { writeSubstituteVersion } from '../lib/substitute-version.mjs';

const MONOREPO = fileURLToPath(new URL('../../../', import.meta.url));
const GITHUB_RELEASE = fileURLToPath(new URL('../lib/github-release.mjs', import.meta.url));

function sugarPublisher() {
  const options = releaseOptions(COMPONENTS.sugarcubes);
  const [, publisher] = options.plugins.find((plugin) => plugin[0] === GITHUB_RELEASE);
  return { options, publisher };
}

describe('GitHub publication', () => {
  let folder;
  before(() => {
    folder = mkdtempSync(join(tmpdir(), 'release-publication-'));
  });
  after(() => rmSync(folder, { recursive: true, force: true }));

  it('publishes SugarCubes only to the repository its release source names', () => {
    const { options, publisher } = sugarPublisher();
    const source = JSON.parse(
      readFileSync(join(MONOREPO, 'Sugar', publisher.releaseSource), 'utf8'),
    ).githubRelease;
    const context = (env, dryRun) => ({
      cwd: join(MONOREPO, 'Sugar'),
      env,
      options: { ...options, dryRun },
    });
    assert.deepEqual(target(publisher, context({ GITHUB_REPOSITORY: source.repository }, false)), {
      repository: source.repository,
    });
    assert.throws(() => target(publisher, context({}, false)), /GITHUB_REPOSITORY/);
    assert.throws(
      () => target(publisher, context({ GITHUB_REPOSITORY: 'someone/fork' }, true)),
      /sends the updater to .*, but this release is tagged in someone\/fork/,
    );
  });

  it('names this monorepo as the SugarCubes release repository', () => {
    const { publisher } = sugarPublisher();
    const source = JSON.parse(
      readFileSync(join(MONOREPO, 'Sugar', publisher.releaseSource), 'utf8'),
    ).githubRelease;
    assert.equal(source.repository, 'Artificial-Sweetener/SugarSubstitute-Rust');
    assert.equal(source.tagPrefix, COMPONENTS.sugarcubes.tagPrefix);
  });

  it('refuses a release source whose tag prefix or asset name differs from the release', () => {
    const { options, publisher } = sugarPublisher();
    const sourcePath = join(folder, 'release_source.json');
    const write = (githubRelease) =>
      writeFileSync(sourcePath, JSON.stringify({ githubRelease }), 'utf8');
    const context = {
      cwd: folder,
      env: { GITHUB_REPOSITORY: 'a/b' },
      options: { ...options, dryRun: true },
    };
    const configured = { ...publisher, releaseSource: 'release_source.json' };
    write({ repository: 'a/b', tagPrefix: 'v', assetName: 'SugarCubes-{version}.zip' });
    assert.throws(() => target(configured, context), /expects tags "v<version>"/);
    write({ repository: 'a/b', tagPrefix: 'sugarcubes-v', assetName: 'pack-{version}.zip' });
    assert.throws(() => target(configured, context), /expects the asset "pack-\{version\}\.zip"/);
    write({
      repository: 'not a repository',
      tagPrefix: 'sugarcubes-v',
      assetName: 'SugarCubes-{version}.zip',
    });
    assert.throws(() => target(configured, context), /does not name a GitHub repository/);
  });

  it('accepts an upload only when GitHub recorded the local file digest', () => {
    const asset = join(folder, 'SugarCubes-1.0.0.zip');
    writeFileSync(asset, 'archive bytes');
    const digest = `sha256:${createHash('sha256').update('archive bytes').digest('hex')}`;
    requireRecordedDigests({ assets: [{ name: 'SugarCubes-1.0.0.zip', digest }] }, [asset]);
    assert.throws(
      () =>
        requireRecordedDigests({ assets: [{ name: 'SugarCubes-1.0.0.zip', digest: null }] }, [
          asset,
        ]),
      /recorded no digest/,
    );
    assert.throws(
      () =>
        requireRecordedDigests(
          { assets: [{ name: 'SugarCubes-1.0.0.zip', digest: 'sha256:00' }] },
          [asset],
        ),
      /recorded sha256:00/,
    );
    assert.throws(() => requireRecordedDigests({ assets: [] }, [asset]), /does not list/);
  });

  it('requires a target repository for a SugarSubstitute release outside a dry run', () => {
    const options = releaseOptions(COMPONENTS.substitute);
    const [, publisher] = options.plugins.find((plugin) => plugin[0] === GITHUB_RELEASE);
    const context = { cwd: folder, env: {}, options: { ...options, dryRun: false } };
    assert.throws(() => target(publisher, context), /GITHUB_REPOSITORY/);
    assert.deepEqual(target(publisher, { ...context, env: { GITHUB_REPOSITORY: 'o/r' } }), {
      repository: 'o/r',
    });
  });
});

describe('SugarSubstitute version files', () => {
  let workspace;
  before(() => {
    workspace = mkdtempSync(join(tmpdir(), 'release-workspace-'));
    mkdirSync(join(workspace, 'crates', 'app'), { recursive: true });
    writeFileSync(
      join(workspace, 'Cargo.toml'),
      '[workspace]\nmembers = ["crates/app"]\n\n[workspace.package]\nversion = "0.1.0"\n\n[workspace.dependencies]\nx = { version = "=0.1.0" }\n',
    );
    writeFileSync(
      join(workspace, 'crates', 'app', 'Cargo.toml'),
      '[package]\nname = "app"\nversion.workspace = true\n',
    );
  });
  after(() => rmSync(workspace, { recursive: true, force: true }));

  it('refuses a lockfile that does not record the declared version', () => {
    writeFileSync(
      join(workspace, 'Cargo.lock'),
      'version = 4\n\n[[package]]\nname = "app"\nversion = "0.0.9"\n',
    );
    assert.throws(() => writeSubstituteVersion(workspace, '0.2.0'), /records app at 0\.0\.9/);
  });

  it('moves only the workspace version and member lock entries', () => {
    writeFileSync(
      join(workspace, 'Cargo.lock'),
      'version = 4\n\n[[package]]\nname = "app"\nversion = "0.1.0"\n',
    );
    writeSubstituteVersion(workspace, '0.2.0');
    const manifest = readFileSync(join(workspace, 'Cargo.toml'), 'utf8');
    assert.match(manifest, /\[workspace\.package\]\nversion = "0\.2\.0"/);
    assert.match(manifest, /x = \{ version = "=0\.1\.0" \}/);
    assert.match(
      readFileSync(join(workspace, 'Cargo.lock'), 'utf8'),
      /name = "app"\nversion = "0\.2\.0"/,
    );
    assert.throws(() => writeSubstituteVersion(workspace, 'v1'), /Expected a release version/);
  });

  it('matches the real workspace layout', () => {
    const real = readFileSync(join(MONOREPO, 'Substitute', 'Cargo.toml'), 'utf8');
    assert.match(real, /^\[workspace\.package\]\nversion = "\d+\.\d+\.\d+"$/m);
  });
});
