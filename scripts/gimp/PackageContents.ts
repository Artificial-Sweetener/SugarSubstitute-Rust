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
/** Assemble the native extension layout that GIMP validates and registers. */
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { nativePluginSources } from './build.js';
import { companionLicenses } from './CompanionLicenses.js';

/** Retain one authoritative native package identity and its compiler-owned payload. */
export interface GimpPackageContents {
  readonly id: string;
  readonly version: string;
  readonly files: ReadonlyMap<string, Buffer>;
}

/** Read the companion's authored identity without accepting a path or XML fragment from dynamic config. */
export function packageIdentity(root: string): { id: string; version: string } {
  const value: unknown = JSON.parse(
    readFileSync(path.join(root, 'integrations/gimp/companion/tauri.conf.json'), 'utf8'),
  );
  if (
    !value ||
    typeof value !== 'object' ||
    !('identifier' in value) ||
    !('version' in value) ||
    typeof value.identifier !== 'string' ||
    !/^[a-z][a-z0-9.-]+(?![\s\S])/.test(value.identifier) ||
    typeof value.version !== 'string' ||
    !/^\d+\.\d+\.\d+(?![\s\S])/.test(value.version)
  )
    throw new Error('Invalid authored GIMP package identity.');
  return { id: value.identifier, version: value.version };
}

/** Name the exact Python entry point, rather than adding a directory containing helper executables to discovery. */
export function packageMetadata(id: string, version: string): string {
  return `<?xml version="1.0" encoding="UTF-8"?>
<component type="addon">
  <id>${id}</id>
  <name>SugarCubes for GIMP</name>
  <summary>Connect GIMP images to SugarCubes and ComfyUI</summary>
  <description><p>Open SugarCubes from GIMP's Windows menu to work with ComfyUI graphs and return generated image layers.</p></description>
  <metadata_license>CC0-1.0</metadata_license>
  <project_license>AGPL-3.0-or-later</project_license>
  <extends>org.gimp.GIMP</extends>
  <requires><id compare="ge" version="3.2.0">org.gimp.GIMP</id></requires>
  <releases><release version="${version}" date="2026-10-25"/></releases>
  <custom><value key="GIMP::plug-in-path">plug-ins/sugarcubes-gimp/sugarcubes-gimp.py</value></custom>
</component>
`;
}

/** Collect only authored runtime inputs and required license resources; source checkouts and local configuration stay out. */
export function packageSources(root: string): GimpPackageContents {
  const { id, version } = packageIdentity(root);
  const prefix = 'plug-ins/sugarcubes-gimp/';
  const files = new Map<string, Buffer>([
    [`${id}.metainfo.xml`, Buffer.from(packageMetadata(id, version))],
    [
      `${prefix}sugarcubes-gimp.py`,
      readFileSync(path.join(root, 'integrations/gimp/entry/sugarcubes-gimp.py')),
    ],
    ['LICENSE.txt', readFileSync(path.join(root, 'LICENSE'))],
    ['THIRD_PARTY_NOTICES.txt', readFileSync(path.join(root, 'third_party/NOTICE.md'))],
    ['THIRD_PARTY_MANIFEST.toml', readFileSync(path.join(root, 'third_party/manifest.toml'))],
    ['licenses/GIMP.txt', readFileSync(path.join(root, 'third_party/licenses/gimp.LICENSE.txt'))],
    [
      'licenses/SugarSubstitute.txt',
      readFileSync(path.join(root, 'third_party/licenses/sugarsubstitute.LICENSE.txt')),
    ],
  ]);
  const marker = readFileSync(path.join(root, 'integrations/gimp/entry/package_marker.py'));
  for (const [name, bytes] of companionLicenses(root)) files.set(name, bytes);
  files.set(prefix + 'integrations/__init__.py', marker);
  files.set(prefix + 'integrations/gimp/__init__.py', marker);
  for (const [name, content] of nativePluginSources(root)) files.set(prefix + name, content);
  if (
    !files.has(`${prefix}integrations/gimp/native/plugin.py`) ||
    !files.has(`${prefix}integrations/gimp/native/native.schema.json`)
  )
    throw new Error('The native plug-in package is incomplete.');
  return { id, version, files };
}

/** Refuse an executable for another architecture before publishing an extension that cannot start. */
export function admitCompanion(bytes: Buffer): void {
  if (bytes.length < 128 || bytes.readUInt16LE(0) !== 0x5a4d)
    throw new Error('The companion is not a Windows executable.');
  const pe = bytes.readUInt32LE(0x3c);
  if (
    pe > bytes.length - 26 ||
    bytes.readUInt32LE(pe) !== 0x4550 ||
    bytes.readUInt16LE(pe + 4) !== 0x8664 ||
    bytes.readUInt16LE(pe + 24) !== 0x20b
  )
    throw new Error('The companion must be a 64-bit Windows executable.');
}

/** Bind the generated archive inventory to its exact native binary and source payload. */
export function packagePayload(
  root: string,
  executable: Buffer,
  development: boolean,
): GimpPackageContents {
  admitCompanion(executable);
  const source = packageSources(root);
  const files = new Map(source.files);
  files.set('plug-ins/sugarcubes-gimp/companion/sugarcubes-gimp-companion.exe', executable);
  files.set(
    'package-inventory.json',
    Buffer.from(
      JSON.stringify(
        {
          id: source.id,
          version: source.version,
          platform: 'windows-x86_64',
          development,
          files: [...files].map(([name, data]) => ({
            name,
            sha256: createHash('sha256').update(data).digest('hex'),
          })),
        },
        null,
        2,
      ) + '\n',
    ),
  );
  return { ...source, files };
}
