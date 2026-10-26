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
/** Build and check the pinned Tauri host without launching any native window. */
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { mkdtempSync, mkdirSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { companionAssets, companionIcon } from './build.js';
import { nativeEnvironment } from './NativeEnvironment.js';

const root = path.resolve(import.meta.dirname, '../..');
const directory = path.join(root, 'integrations/gimp/companion');
const command = process.argv[2];
const target = process.argv[4];
const checks =
  command === 'build'
    ? [
        [
          'build',
          '--locked',
          '--features',
          'desktop',
          ...(process.argv.includes('--release') ? ['--release'] : []),
        ],
      ]
    : command === 'check'
      ? [
          ['fmt', '--all', '--', '--check'],
          ['clippy', '--locked', '--all-targets', '--all-features', '--', '-D', 'warnings'],
        ]
      : command === 'test' && process.argv[3] === '--test' && target && /^[\w-]+$/.test(target)
        ? [['test', '--locked', '--features', 'desktop', '--test', target]]
        : null;
if (!checks)
  throw new Error('Choose build, check or one declared test target for the GIMP native host.');
/** Stage native validation inputs separately from published packages and own their cleanup. */
async function main(): Promise<void> {
  const stagingRoot = path.join(root, '.generated/gimp');
  mkdirSync(stagingRoot, { recursive: true });
  const staging = mkdtempSync(path.join(stagingRoot, 'native-assets-'));
  if (!realpathSync(staging).startsWith(realpathSync(root) + path.sep))
    throw new Error('Native staging escaped the workspace.');
  try {
    for (const [name, content] of await companionAssets(root))
      writeFileSync(path.join(staging, name), content);
    const icon = path.join(staging, 'companion.ico');
    writeFileSync(icon, companionIcon(root));
    const environment = nativeEnvironment();
    environment.SUGARCUBES_GIMP_GUEST = path.join(staging, 'guest.js');
    environment.TAURI_CONFIG = JSON.stringify({
      // Tauri interprets a drive-qualified string as a URL rather than an asset directory.
      build: { frontendDist: path.relative(directory, staging).replaceAll(path.sep, '/') },
      bundle: { icon: [icon] },
    });
    for (const args of checks ?? []) {
      const result = spawnSync('cargo', ['+1.93.1', ...args], {
        cwd: directory,
        stdio: 'inherit',
        env: environment,
      });
      if (result.error) throw result.error;
      if (result.status !== 0) {
        process.exitCode = result.status ?? 1;
        break;
      }
    }
  } finally {
    rmSync(staging, { recursive: true, force: true });
  }
}
main().catch((error: unknown) => {
  console.error(error);
  process.exitCode = 1;
});
