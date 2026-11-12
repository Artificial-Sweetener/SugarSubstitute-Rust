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
/** Refresh the centrally recorded companion notices from the pinned, offline Windows dependency resolution. */
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import {
  cargoNotices,
  renderCargoNotices,
  webviewLoaderNotices,
  webview2LoaderSha256,
  type CrateNotices,
} from './CargoLicenseInventory.js';
import {
  companionLock,
  companionManifest,
  companionNoticeFile,
  companionNoticeIndex,
  licenseHash,
} from './CompanionLicenses.js';

if (process.argv.slice(2).join(' ') !== '--write')
  throw new Error('Choose --write to regenerate the pinned native notices.');
const root = path.resolve(import.meta.dirname, '../..');
const metadata: unknown = JSON.parse(
  execFileSync(
    'cargo',
    [
      '+1.93.1',
      'metadata',
      '--locked',
      '--offline',
      '--features',
      'desktop',
      '--filter-platform',
      'x86_64-pc-windows-msvc',
      '--format-version',
      '1',
    ],
    {
      cwd: path.join(root, 'integrations/gimp/companion'),
      encoding: 'utf8',
      windowsHide: true,
      maxBuffer: 32 * 1024 * 1024,
    },
  ),
);
const components: CrateNotices[] = [...cargoNotices(root, metadata)];
components.push(webviewLoaderNotices(root, metadata));
const notices = renderCargoNotices(components);
const index = {
  target: 'x86_64-pc-windows-msvc',
  features: ['desktop'],
  lockSha256: licenseHash(readFileSync(path.join(root, companionLock))),
  manifestSha256: licenseHash(readFileSync(path.join(root, companionManifest))),
  noticesSha256: licenseHash(notices.text),
  webview2LoaderSha256,
  components: notices.components,
};
writeFileSync(path.join(root, companionNoticeFile), notices.text);
writeFileSync(path.join(root, companionNoticeIndex), JSON.stringify(index, null, 2) + '\n');
console.log(`Recorded ${components.length} pinned native dependency notices.`);
