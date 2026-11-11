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
/** Verify the checked-in companion notices against the exact authored native dependency inputs. */
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';

export const companionNoticeFile = 'third_party/licenses/gimp-companion-dependencies.txt';
export const companionNoticeIndex = 'third_party/gimp-companion-licenses.json';
export const companionManifest = 'integrations/gimp/companion/Cargo.toml';
export const companionLock = 'integrations/gimp/companion/Cargo.lock';

/** Fingerprint bytes without normalizing upstream license text or native dependency inputs. */
export function licenseHash(value: Buffer): string {
  return createHash('sha256').update(value).digest('hex');
}

/** Reject stale or altered notices before a development or release executable enters a package. */
export function companionLicenses(root: string): ReadonlyMap<string, Buffer> {
  const index = readFileSync(path.join(root, companionNoticeIndex));
  const value: unknown = JSON.parse(index.toString());
  const notices = readFileSync(path.join(root, companionNoticeFile));
  if (
    !value ||
    typeof value !== 'object' ||
    !('lockSha256' in value) ||
    !('manifestSha256' in value) ||
    !('noticesSha256' in value) ||
    !('components' in value) ||
    !Array.isArray(value.components) ||
    value.components.length === 0 ||
    value.lockSha256 !== licenseHash(readFileSync(path.join(root, companionLock))) ||
    value.manifestSha256 !== licenseHash(readFileSync(path.join(root, companionManifest))) ||
    value.noticesSha256 !== licenseHash(notices)
  )
    throw new Error(
      'Regenerate the pinned companion notices with scripts/gimp/licenses.ts --write.',
    );
  return new Map([
    ['licenses/companion-dependencies.txt', notices],
    ['licenses/companion-dependencies.json', index],
  ]);
}
