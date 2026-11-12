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
/** Validate a Photoshop package before it reaches the host or distribution tooling. */
import { existsSync, readFileSync, readdirSync, lstatSync } from 'node:fs';
import path from 'node:path';

/** Describe the validated manifest facts required by build tooling. */
export interface PhotoshopManifest {
  readonly id: string;
  readonly version: string;
  readonly main: string;
}

/** Require string-keyed external configuration before accessing fields. */
export function record(value: unknown, context: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value))
    throw new Error(`${context} must be an object.`);
  return value as Record<string, unknown>;
}

/** Require a nonempty configuration string. */
export function text(value: unknown, context: string): string {
  if (typeof value !== 'string' || !value.trim())
    throw new Error(`${context} must be a nonempty string.`);
  return value;
}

/** Keep package references inside their declared root on Windows and POSIX. */
export function packagePath(root: string, relative: string): string {
  if (
    relative.includes('\\') ||
    relative.includes(':') ||
    relative.split('/').some((part) => part === '..') ||
    path.isAbsolute(relative)
  )
    throw new Error(`Unsafe package path: ${relative}`);
  const target = path.resolve(root, relative);
  if (!target.startsWith(`${path.resolve(root)}${path.sep}`))
    throw new Error(`Unsafe package path: ${relative}`);
  return target;
}

/** Validate manifest identity, Photoshop compatibility, permissions and static references. */
export function validateManifest(value: unknown): PhotoshopManifest {
  const manifest = record(value, 'manifest');
  if (manifest.manifestVersion !== 5) throw new Error('Photoshop requires manifestVersion 5.');
  const id = text(manifest.id, 'manifest.id');
  text(manifest.name, 'manifest.name');
  const version = text(manifest.version, 'manifest.version');
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error('Plugin version must be x.y.z.');
  const host = record(manifest.host, 'manifest.host');
  if (host.app !== 'PS' || !/^\d+\.\d+(?:\.\d+)?$/.test(text(host.minVersion, 'host.minVersion')))
    throw new Error('Declare one Photoshop host and an explicit minimum version.');
  const main = text(manifest.main, 'manifest.main');
  packagePath(process.cwd(), main);
  if (!main.endsWith('.html')) throw new Error('The panel entry must be an HTML file.');
  if (!Array.isArray(manifest.entrypoints) || !manifest.entrypoints.length)
    throw new Error('Declare panel entrypoints.');
  const ids = new Set<string>();
  for (const value of manifest.entrypoints) {
    const entry = record(value, 'entrypoint');
    const entryId = text(entry.id, 'entrypoint.id');
    if (ids.has(entryId)) throw new Error('Entrypoint IDs must be unique.');
    ids.add(entryId);
    if (entry.type !== 'panel') throw new Error('This build target packages panel entrypoints.');
  }
  const permissions = record(manifest.requiredPermissions, 'requiredPermissions');
  for (const name of ['network', 'webview']) {
    const value = permissions[name];
    if (value === undefined) continue;
    const permission = record(value, name);
    if (
      !Array.isArray(permission.domains) ||
      !permission.domains.length ||
      permission.domains.some(
        (domain: unknown) => typeof domain !== 'string' || !/^https?:\/\/[^*\s]+$/.test(domain),
      )
    )
      throw new Error(`${name} requires explicit HTTP(S) origins without wildcards.`);
    for (const domain of permission.domains) {
      const url = new URL(text(domain, 'domain'));
      if (
        url.username ||
        url.password ||
        (url.pathname !== '/' && url.pathname !== '') ||
        url.search ||
        url.hash
      )
        throw new Error('Permissions must name origins without credentials or paths.');
    }
    if (name === 'webview' && permission.allow !== 'yes')
      throw new Error('WebView permission must explicitly allow access.');
  }
  if (permissions.localFileSystem === 'fullAccess')
    throw new Error('Full filesystem access requires a reviewed build policy change.');
  return { id, version, main };
}

/** Inventory package files while rejecting linked files and directories. */
export function packageFiles(root: string): string[] {
  if (!existsSync(root)) return [];
  return readdirSync(root)
    .flatMap((name) => {
      const file = path.join(root, name);
      const info = lstatSync(file);
      if (info.isSymbolicLink()) throw new Error(`Linked package resources are forbidden: ${name}`);
      return info.isDirectory() ? packageFiles(file) : [file];
    })
    .sort();
}

/** Prove the staged package is self-contained and excludes local and development inputs. */
export function validatePackage(root: string, release: boolean): PhotoshopManifest {
  const payload: unknown = JSON.parse(readFileSync(path.join(root, 'manifest.json'), 'utf8'));
  const manifest = validateManifest(payload);
  const requireFile = (relative: string): void => {
    const file = packagePath(root, relative.split(/[?#]/)[0] ?? relative);
    if (!existsSync(file) || !lstatSync(file).isFile())
      throw new Error(`Missing package resource: ${relative}`);
  };
  requireFile(manifest.main);
  const references = (value: unknown): void => {
    if (Array.isArray(value)) for (const item of value) references(item);
    else if (value && typeof value === 'object')
      for (const [key, item] of Object.entries(value)) {
        if (key === 'path') requireFile(text(item, 'resource.path'));
        else references(item);
      }
  };
  references(payload);
  for (const file of packageFiles(root)) {
    const relative = path.relative(root, file).replaceAll('\\', '/');
    if (
      /(?:^|\/)(?:\.|node_modules|tests?\/)|\.local\.|\.(?:ts|tsx|log|ccx)$/.test(relative) ||
      (release && relative.endsWith('.map'))
    )
      throw new Error(`Development or local file in plugin package: ${relative}`);
    if (!file.endsWith('.html') && !file.endsWith('.css')) continue;
    const content = readFileSync(file, 'utf8');
    if (release && /uxpallowinspector\s*=\s*["']?true/i.test(content))
      throw new Error('Release output enables the WebView inspector.');
    const resources = file.endsWith('.css')
      ? content.matchAll(/url\(\s*["']?([^\s"')]+)["']?\s*\)/gi)
      : content.matchAll(/(?:src|href)\s*=\s*["']([^"']+)["']/gi);
    for (const match of resources) {
      const reference = match[1];
      if (reference && !/^(?:https?:|data:|#)/.test(reference)) {
        const directory = path.posix.dirname(relative);
        requireFile(directory === '.' ? reference : `${directory}/${reference}`);
      }
    }
  }
  return manifest;
}
