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
/** Collect complete upstream license resources from the locked Windows desktop dependency closure. */
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { licenseHash } from './CompanionLicenses.js';

interface CargoPackage {
  readonly id: string;
  readonly name: string;
  readonly version: string;
  readonly license: string;
  readonly repository: string | null;
  readonly manifest_path: string;
}

export interface LicenseResource {
  readonly source: string;
  readonly bytes: Buffer;
}

export interface CrateNotices {
  readonly name: string;
  readonly version: string;
  readonly license: string;
  readonly repository: string | null;
  readonly source: string;
  readonly resources: readonly LicenseResource[];
}

export interface IndexedCrateNotices extends Omit<CrateNotices, 'resources'> {
  readonly resources: readonly { readonly source: string; readonly sha256: string }[];
}

export const webview2LoaderSha256 =
  '89c6b872783b8f6c3cedbff618adb42082d455c615453ae10cfc753f1e8f25d8';

/** Admit only a Cargo package with explicit identity, licensing and a registry-backed source. */
function cargoPackage(value: unknown): CargoPackage {
  if (
    !value ||
    typeof value !== 'object' ||
    !('id' in value) ||
    typeof value.id !== 'string' ||
    !('name' in value) ||
    typeof value.name !== 'string' ||
    !/^[\w-]+$/.test(value.name) ||
    !('version' in value) ||
    typeof value.version !== 'string' ||
    !('license' in value) ||
    typeof value.license !== 'string' ||
    !value.license.trim() ||
    !('repository' in value) ||
    !(value.repository === null || typeof value.repository === 'string') ||
    !('manifest_path' in value) ||
    typeof value.manifest_path !== 'string' ||
    !('source' in value) ||
    value.source !== 'registry+https://github.com/rust-lang/crates.io-index'
  )
    throw new Error('A native dependency has incomplete identity or license metadata.');
  return {
    id: value.id,
    name: value.name,
    version: value.version,
    license: value.license,
    repository: value.repository,
    manifest_path: value.manifest_path,
  };
}

/** Refuse incomplete resolution instead of silently dropping a dependency or an unknown source. */
function closure(metadata: unknown): readonly CargoPackage[] {
  if (
    !metadata ||
    typeof metadata !== 'object' ||
    !('packages' in metadata) ||
    !Array.isArray(metadata.packages) ||
    !('resolve' in metadata) ||
    !metadata.resolve ||
    typeof metadata.resolve !== 'object' ||
    !('root' in metadata.resolve) ||
    typeof metadata.resolve.root !== 'string' ||
    !('nodes' in metadata.resolve) ||
    !Array.isArray(metadata.resolve.nodes)
  )
    throw new Error('Invalid Cargo license metadata.');
  const edges = new Map<string, readonly string[]>();
  for (const node of metadata.resolve.nodes) {
    if (
      !node ||
      typeof node !== 'object' ||
      !('id' in node) ||
      typeof node.id !== 'string' ||
      !('dependencies' in node) ||
      !Array.isArray(node.dependencies) ||
      !node.dependencies.every((id: unknown): id is string => typeof id === 'string')
    )
      throw new Error('Invalid Cargo dependency resolution.');
    edges.set(node.id, node.dependencies);
  }
  const visited = new Set<string>();
  const pending = [metadata.resolve.root];
  while (pending.length) {
    const id = pending.pop();
    if (!id || visited.has(id)) continue;
    visited.add(id);
    const dependencies = edges.get(id);
    if (!dependencies) throw new Error('Missing Cargo dependency resolution.');
    pending.push(...dependencies);
  }
  visited.delete(metadata.resolve.root);
  const result: CargoPackage[] = [];
  for (const value of metadata.packages) {
    if (!value || typeof value !== 'object' || !('id' in value) || typeof value.id !== 'string')
      throw new Error('Invalid Cargo package identity.');
    if (visited.delete(value.id)) result.push(cargoPackage(value));
  }
  if (visited.size || !result.length) throw new Error('The Cargo license closure is incomplete.');
  return result.sort((a, b) =>
    `${a.name}@${a.version}`.localeCompare(`${b.name}@${b.version}`, 'en'),
  );
}

/** Keep every license, copyright and notice shipped by a crate, including embedded upstream components. */
function crateResources(directory: string, relative = ''): LicenseResource[] {
  const result: LicenseResource[] = [];
  for (const entry of readdirSync(path.join(directory, relative), { withFileTypes: true })) {
    const source = path.posix.join(relative.replaceAll(path.sep, '/'), entry.name);
    if (entry.isSymbolicLink()) throw new Error('Native license collection does not follow links.');
    if (entry.isDirectory()) result.push(...crateResources(directory, source));
    else if (entry.isFile() && /^(licen[cs]e|copying|notice|copyright)([.\-_]|$)/i.test(entry.name))
      result.push({ source, bytes: readFileSync(path.join(directory, source)) });
  }
  return result.sort((a, b) => a.source.localeCompare(b.source, 'en'));
}

/** Supply explicitly pinned repository-level licenses omitted from published crate archives. */
function missingResources(root: string, name: string, version: string): LicenseResource[] {
  const overrides: Readonly<Record<string, { file: string; source: string }>> = {
    'alloc-stdlib@0.3.0': {
      file: 'alloc-stdlib.BSD-3-Clause.txt',
      source:
        'https://raw.githubusercontent.com/dropbox/rust-alloc-no-stdlib/0a81fd6928ea3b33c8cd484aa4575d50ffb98012/LICENSE',
    },
    'selectors@0.38.0': {
      file: 'MPL-2.0.txt',
      source: 'https://www.mozilla.org/media/MPL/2.0/index.txt',
    },
    'webview2-com@0.39.1': {
      file: 'webview2-rs.MIT.txt',
      source:
        'https://raw.githubusercontent.com/wravery/webview2-rs/edc2caf886175ccaebe86078c9cfe1ae2a187328/LICENSE',
    },
    'webview2-com-sys@0.39.1': {
      file: 'webview2-rs.MIT.txt',
      source:
        'https://raw.githubusercontent.com/wravery/webview2-rs/edc2caf886175ccaebe86078c9cfe1ae2a187328/LICENSE',
    },
    'webview2-com-macros@0.8.1': {
      file: 'webview2-rs.MIT.txt',
      source:
        'https://raw.githubusercontent.com/wravery/webview2-rs/dffa41a8a46d3f5565eefbff2de57d38d399f158/LICENSE',
    },
  };
  const override = overrides[`${name}@${version}`];
  if (!override) throw new Error(`Missing upstream license text for ${name}@${version}.`);
  return [
    {
      source: override.source,
      bytes: readFileSync(path.join(root, 'third_party/licenses/gimp-companion', override.file)),
    },
  ];
}

/** Capture exact source downloads and complete texts without leaking local Cargo cache paths. */
export function cargoNotices(root: string, metadata: unknown): readonly CrateNotices[] {
  return closure(metadata).map((dependency) => {
    const resources = crateResources(path.dirname(dependency.manifest_path));
    const admitted = resources.length
      ? resources
      : missingResources(root, dependency.name, dependency.version);
    if (admitted.some((resource) => !resource.bytes.length))
      throw new Error('Empty native license resource.');
    return {
      name: dependency.name,
      version: dependency.version,
      license: dependency.license,
      repository: dependency.repository,
      source: `https://crates.io/api/v1/crates/${dependency.name}/${dependency.version}/download`,
      resources: admitted,
    };
  });
}

/** Bind the additional SDK notices to the exact Microsoft loader embedded by the locked bindings crate. */
export function webviewLoaderNotices(root: string, metadata: unknown): CrateNotices {
  const dependency = closure(metadata).find((item) => item.name === 'webview2-com-sys');
  if (
    !dependency ||
    dependency.version !== '0.39.1' ||
    licenseHash(
      readFileSync(
        path.join(path.dirname(dependency.manifest_path), 'x64/WebView2LoaderStatic.lib'),
      ),
    ) !== webview2LoaderSha256
  )
    throw new Error(
      'Review the Microsoft SDK loader provenance before generating companion notices.',
    );
  return {
    name: 'Microsoft.Web.WebView2 loader',
    version: '1.0.3800.47',
    license: 'BSD-3-Clause',
    repository: 'https://www.nuget.org/packages/Microsoft.Web.WebView2/1.0.3800.47',
    source:
      'https://api.nuget.org/v3-flatcontainer/microsoft.web.webview2/1.0.3800.47/microsoft.web.webview2.1.0.3800.47.nupkg',
    resources: ['LICENSE.txt', 'NOTICE.txt'].map((name) => ({
      source: name,
      bytes: readFileSync(
        path.join(root, 'third_party/licenses/gimp-companion', `webview2-sdk.${name}`),
      ),
    })),
  };
}

/** Preserve upstream bytes as UTF-8 text and fingerprint each independently addressable resource. */
export function renderCargoNotices(components: readonly CrateNotices[]): {
  text: Buffer;
  components: readonly IndexedCrateNotices[];
} {
  const sections: string[] = [
    'SugarCubes GIMP companion dependency notices\nSource archives are linked for each unmodified dependency.\n',
  ];
  for (const component of components) {
    sections.push(
      `\n${'='.repeat(72)}\n${component.name} ${component.version}\nLicense: ${component.license}\nSource: ${component.source}\n`,
    );
    for (const resource of component.resources) {
      const text = resource.bytes.toString('utf8');
      if (!Buffer.from(text).equals(resource.bytes))
        throw new Error('A native license is not UTF-8 text.');
      sections.push(`\n--- ${resource.source} ---\n${text}\n`);
    }
  }
  return {
    text: Buffer.from(sections.join('')),
    components: components.map(({ resources, ...component }) => ({
      ...component,
      resources: resources.map(({ source, bytes }) => ({ source, sha256: licenseHash(bytes) })),
    })),
  };
}
