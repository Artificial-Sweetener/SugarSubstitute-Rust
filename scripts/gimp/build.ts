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
/** Compile the local Tauri shell while consuming SugarCubes' existing GIMP palette owner. */
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { build } from 'esbuild';
import { gimpTokenDeclarations } from '../../frontend/comfyui/ui/surface/appearance/GimpThemeTokens.js';
import { verifyNativeContract } from './NativeContract.js';

/** Wrap the licensed SugarSubstitute application PNG in an ICO container without altering its pixels. */
export function companionIcon(root: string): Buffer {
  const png = readFileSync(path.join(root, 'frontend/static/orb/app_icon_256.png'));
  if (png.readUInt32BE(16) !== 256 || png.readUInt32BE(20) !== 256)
    throw new Error('The companion icon requires the existing 256px application asset.');
  const header = Buffer.alloc(22);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(1, 4);
  header.writeUInt16LE(1, 10);
  header.writeUInt16LE(32, 12);
  header.writeUInt32LE(png.length, 14);
  header.writeUInt32LE(22, 18);
  return Buffer.concat([header, png]);
}

/** Package native adapter modules under their import identity without runtime path injection. */
export function nativePluginSources(root: string): ReadonlyMap<string, Buffer> {
  const source = path.join(root, 'integrations/gimp/native');
  const files = new Map<string, Buffer>();
  for (const item of readdirSync(source, { recursive: true, withFileTypes: true })) {
    if (!item.isFile() || (!item.name.endsWith('.py') && item.name !== 'native.schema.json'))
      continue;
    const file = path.join(item.parentPath, item.name);
    const relative = path.relative(source, file).split(path.sep).join('/');
    files.set(`integrations/gimp/native/${relative}`, readFileSync(file));
  }
  return files;
}

/** Return deterministic browser assets independently of native compilation and application state. */
export async function companionAssets(root: string): Promise<ReadonlyMap<string, string>> {
  const source = path.join(root, 'integrations/gimp');
  const options = {
    bundle: true,
    write: false,
    platform: 'browser' as const,
    target: 'es2022',
    tsconfig: path.join(root, 'tsconfig.gimp.json'),
    legalComments: 'inline' as const,
  };
  const bundle = await build({
    ...options,
    entryPoints: [path.join(source, 'src/presentation/main.ts')],
    format: 'esm',
  });
  const guest = await build({
    ...options,
    entryPoints: [path.join(source, 'src/infrastructure/ComfyGuest.ts')],
    format: 'iife',
  });
  const output = bundle.outputFiles?.[0];
  const guestOutput = guest.outputFiles?.[0];
  if (!output || !guestOutput) throw new Error('The GIMP companion produced no JavaScript.');
  return new Map([
    ['index.html', readFileSync(path.join(source, 'static/index.html'), 'utf8')],
    ['shell.css', readFileSync(path.join(source, 'static/shell.css'), 'utf8')],
    ['palette.css', `:root{${gimpTokenDeclarations(true)}}\n`],
    ['main.js', output.text],
    ['guest.js', guestOutput.text],
    ['LICENSE.txt', readFileSync(path.join(root, 'LICENSE'), 'utf8')],
    ['THIRD_PARTY_NOTICES.txt', readFileSync(path.join(root, 'third_party/NOTICE.md'), 'utf8')],
    [
      'THIRD_PARTY_MANIFEST.toml',
      readFileSync(path.join(root, 'third_party/manifest.toml'), 'utf8'),
    ],
    [
      'GIMP_LICENSE.txt',
      readFileSync(path.join(root, 'third_party/licenses/gimp.LICENSE.txt'), 'utf8'),
    ],
    [
      'SUBSTITUTE_LICENSE.txt',
      readFileSync(path.join(root, 'third_party/licenses/sugarsubstitute.LICENSE.txt'), 'utf8'),
    ],
  ]);
}

/** Stage compiler-owned assets, or validate them without rewriting build output. */
async function main(): Promise<void> {
  const root = path.resolve(import.meta.dirname, '../..');
  await verifyNativeContract(root);
  const output = path.join(root, 'build/gimp/companion');
  const check = process.argv.includes('--check');
  for (const [name, source] of nativePluginSources(root)) {
    if (!source.length) throw new Error(`Empty native plug-in source: ${name}`);
    if (!check) {
      const file = path.join(root, 'build/gimp/plugin', name);
      mkdirSync(path.dirname(file), { recursive: true });
      writeFileSync(file, source);
    }
  }
  if (!check) mkdirSync(output, { recursive: true });
  const icon = companionIcon(root);
  if (!check) writeFileSync(path.join(output, 'companion.ico'), icon);
  for (const [name, content] of await companionAssets(root)) {
    if (!content.trim()) throw new Error(`Empty companion asset: ${name}`);
    if (!check) writeFileSync(path.join(output, name), content);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)
  main().catch((error: unknown) => {
    console.error(error);
    process.exitCode = 1;
  });
