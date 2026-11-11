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
/** Prove GEX metadata names the actual entry point and packages normal Python imports with exact binary provenance. */
import { createHash } from 'node:crypto';
import {
  admitCompanion,
  packagePayload,
  packageSources,
} from '../../../scripts/gimp/PackageContents.js';
import { pluginPackageContents } from '../../../scripts/gimp/PluginPackageContents.js';
import { admitPackageOutput } from '../../../scripts/gimp/PackageArchive.js';

/** Model only a native compiler's PE header; real installation tests use the actual companion. */
function executable(): Buffer {
  const result = Buffer.alloc(256);
  result.writeUInt16LE(0x5a4d, 0);
  result.writeUInt32LE(128, 0x3c);
  result.writeUInt32LE(0x4550, 128);
  result.writeUInt16LE(0x8664, 132);
  result.writeUInt16LE(0x20b, 152);
  return result;
}

test('packages only the declared native entry and its self-contained namespace', () => {
  const source = packageSources(process.cwd());
  const prefix = 'plug-ins/sugarcubes-gimp/';
  expect(source.files.get(`${source.id}.metainfo.xml`)?.toString()).toContain(
    '<value key="GIMP::plug-in-path">plug-ins/sugarcubes-gimp/sugarcubes-gimp.py</value>',
  );
  for (const name of [
    'sugarcubes-gimp.py',
    'integrations/__init__.py',
    'integrations/gimp/__init__.py',
    'integrations/gimp/native/plugin.py',
    'integrations/gimp/native/native.schema.json',
  ])
    expect(source.files.has(prefix + name)).toBe(true);
  expect(
    [...source.files.keys()].some((name) => /node_modules|tests\/|\.local\.|\.ts$/.test(name)),
  ).toBe(false);
  expect([...source.files.values()].every((data) => data.length > 0)).toBe(true);
});

test('binds package provenance to the exact supplied compiler output', () => {
  const bytes = executable();
  const packaged = packagePayload(process.cwd(), bytes, true);
  const name = 'plug-ins/sugarcubes-gimp/companion/sugarcubes-gimp-companion.exe';
  expect(packaged.files.get(name)).toBe(bytes);
  const inventory: unknown = JSON.parse(
    packaged.files.get('package-inventory.json')?.toString() ?? 'null',
  );
  expect(inventory).toMatchObject({
    platform: 'windows-x86_64',
    development: true,
    files: expect.arrayContaining([
      { name, sha256: createHash('sha256').update(bytes).digest('hex') },
    ]),
  });
});

test('standard plug-in installation contains the same runtime bytes and notices without extension activation', () => {
  const extension = packagePayload(process.cwd(), executable(), false);
  const plugin = pluginPackageContents(extension, false);
  const prefix = 'plug-ins/sugarcubes-gimp/';
  for (const [name, bytes] of extension.files) {
    if (name === `${extension.id}.metainfo.xml` || name === 'package-inventory.json') continue;
    expect(plugin.files.get(name.startsWith(prefix) ? name.slice(prefix.length) : name)).toBe(
      bytes,
    );
  }
  expect(plugin.files.has(`${extension.id}.metainfo.xml`)).toBe(false);
  expect(plugin.files.has('sugarcubes-gimp.py')).toBe(true);
  expect(plugin.files.has('integrations/gimp/native/geometry.py')).toBe(true);
  expect(plugin.files.has('licenses/GIMP.txt')).toBe(true);
  expect(plugin.files.has('companion/sugarcubes-gimp-companion.exe')).toBe(true);
  const inventory: unknown = JSON.parse(
    plugin.files.get('package-inventory.json')?.toString() ?? 'null',
  );
  expect(inventory).toMatchObject({
    id: extension.id,
    development: false,
    files: expect.arrayContaining([
      {
        name: 'sugarcubes-gimp.py',
        sha256: createHash('sha256')
          .update(plugin.files.get('sugarcubes-gimp.py') ?? '')
          .digest('hex'),
      },
    ]),
  });
  expect(JSON.stringify(inventory)).not.toContain('plug-ins/');
});

test('refuses incomplete standard distributions and package output on other drives', () => {
  const extension = packagePayload(process.cwd(), executable(), true);
  const files = new Map(extension.files);
  files.delete('plug-ins/sugarcubes-gimp/companion/sugarcubes-gimp-companion.exe');
  expect(() => pluginPackageContents({ ...extension, files }, true)).toThrow('incomplete');
  for (const output of ['relative', 'C:\\temporary', 'E:\\project'])
    expect(() => admitPackageOutput(output)).toThrow('D-drive');
  expect(() => admitPackageOutput('D:\\SugarCubes\\packages')).not.toThrow();
});

test.each([Buffer.alloc(0), Buffer.alloc(256)])('rejects nonnative package payloads', (bytes) => {
  expect(() => admitCompanion(bytes)).toThrow('Windows executable');
});

test('rejects wrong architecture and corrupt header offsets', () => {
  const bytes = executable();
  bytes.writeUInt16LE(0x14c, 132);
  expect(() => admitCompanion(bytes)).toThrow('64-bit');
  bytes.writeUInt32LE(0xffffffff, 0x3c);
  expect(() => admitCompanion(bytes)).toThrow('64-bit');
});
