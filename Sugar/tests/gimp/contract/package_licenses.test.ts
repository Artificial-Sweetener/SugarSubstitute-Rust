/** @jest-environment node */
//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Prove native packages carry exact upstream notices and refuse stale dependency inventories. */
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import {
  cargoNotices,
  renderCargoNotices,
  webviewLoaderNotices,
} from '../../../scripts/gimp/CargoLicenseInventory.js';
import {
  companionLicenses,
  companionLock,
  companionManifest,
  companionNoticeFile,
  companionNoticeIndex,
  licenseHash,
} from '../../../scripts/gimp/CompanionLicenses.js';

const directories = new Set<string>();

/** Keep synthetic registry data isolated from the installed compiler and production license cache. */
function directory(): string {
  const result = mkdtempSync(path.join(tmpdir(), 'gimp-licenses-'));
  directories.add(result);
  return result;
}

/** Write a small admitted resource without assuming an existing package layout. */
function resource(root: string, name: string, value: string | Buffer): void {
  const target = path.join(root, name);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, value);
}

/** Describe one reachable dependency and one unrelated package in Cargo's external representation. */
function metadata(root: string) {
  return {
    packages: [
      {
        id: 'registry#example',
        name: 'example',
        version: '1.0.0',
        license: 'MIT',
        repository: 'https://example.invalid/source',
        manifest_path: path.join(root, 'crate/Cargo.toml'),
        source: 'registry+https://github.com/rust-lang/crates.io-index',
      },
      { id: 'unused', source: null },
    ],
    resolve: {
      root: 'companion',
      nodes: [
        { id: 'companion', dependencies: ['registry#example'] },
        { id: 'registry#example', dependencies: [] },
      ],
    },
  };
}

afterEach(() => {
  for (const root of directories) rmSync(root, { recursive: true, force: true });
  directories.clear();
});

test('ships upstream license and nested notices for every reachable crate without exposing local paths', () => {
  const root = directory();
  resource(root, 'crate/LICENSE', 'Copyright upstream\nMIT permission notice\n');
  resource(root, 'crate/vendor/NOTICE.txt', 'Embedded component attribution\n');
  const components = cargoNotices(root, metadata(root));
  const notices = renderCargoNotices(components);
  expect(components).toHaveLength(1);
  expect(notices.text.toString()).toContain('Embedded component attribution');
  expect(notices.text.toString()).toContain(
    'https://crates.io/api/v1/crates/example/1.0.0/download',
  );
  expect(JSON.stringify(notices.components)).not.toContain(root);
  expect(notices.components[0]?.resources).toHaveLength(2);
});

test('refuses incomplete resolution, unknown source provenance, absent and empty license texts', () => {
  const root = directory();
  mkdirSync(path.join(root, 'crate'));
  expect(() => cargoNotices(root, metadata(root))).toThrow('Missing upstream license');
  resource(root, 'crate/LICENSE', '');
  expect(() => cargoNotices(root, metadata(root))).toThrow('Empty');
  const missing = metadata(root);
  missing.resolve.nodes[0]?.dependencies.push('missing');
  expect(() => cargoNotices(root, missing)).toThrow('Missing Cargo');
  const unknown = metadata(root);
  const first = unknown.packages[0];
  if (!first) throw new Error('Missing test package');
  first.source = 'git+https://example.invalid';
  expect(() => cargoNotices(root, unknown)).toThrow('identity or license');
});

test('requires a renewed license review when the embedded Microsoft loader changes', () => {
  const root = directory();
  const value = metadata(root);
  const first = value.packages[0];
  if (!first || !('name' in first)) throw new Error('Missing test package');
  first.name = 'webview2-com-sys';
  first.version = '0.39.1';
  resource(root, 'crate/x64/WebView2LoaderStatic.lib', 'Different embedded SDK');
  expect(() => webviewLoaderNotices(root, value)).toThrow('loader provenance');
});

test.each([companionLock, companionManifest, companionNoticeFile])(
  'rejects changed %s instead of shipping stale notices',
  (changed) => {
    const root = directory();
    for (const name of [companionLock, companionManifest, companionNoticeFile])
      resource(root, name, name);
    resource(
      root,
      companionNoticeIndex,
      JSON.stringify({
        lockSha256: licenseHash(Buffer.from(companionLock)),
        manifestSha256: licenseHash(Buffer.from(companionManifest)),
        noticesSha256: licenseHash(Buffer.from(companionNoticeFile)),
        components: [{ name: 'example' }],
      }),
    );
    expect(companionLicenses(root).size).toBe(2);
    resource(root, changed, 'Changed after notices were generated');
    expect(() => companionLicenses(root)).toThrow('Regenerate');
  },
);

test('packages the current pinned inventory and complete upstream texts', () => {
  const files = companionLicenses(process.cwd());
  expect(files.get('licenses/companion-dependencies.txt')?.toString()).toContain(
    'Microsoft.Web.WebView2 loader 1.0.3800.47',
  );
  expect(files.get('licenses/companion-dependencies.txt')?.toString()).toContain(
    'Mozilla Public License Version 2.0',
  );
  const index = readFileSync(path.join(process.cwd(), companionNoticeIndex), 'utf8');
  expect(index).not.toMatch(/[A-Z]:\\\\|manifest_path/);
});
