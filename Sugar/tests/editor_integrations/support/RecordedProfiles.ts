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
/** Compare a host's current behaviour with its newest recorded profile, for the host's own proof. */
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import type {
  HostProfileSlot,
  RecordedHostProfile,
} from '../../../integrations/shared/conformance/HostProfiles.js';

/** Where recorded profiles live, one folder per host and one file per release. */
export const PROFILE_ROOT = 'integrations/shared/conformance/profiles';

/** Order release names numerically, part by part. */
function compareReleases(left: string, right: string): number {
  const parts = (value: string): number[] => value.split(/[.-]/).map((part) => Number(part) || 0);
  const [a, b] = [parts(left), parts(right)];
  for (let index = 0; index < Math.max(a.length, b.length); index += 1) {
    const difference = (a[index] ?? 0) - (b[index] ?? 0);
    if (difference) return difference;
  }
  return 0;
}

/** Read every recorded profile of one host, oldest release first. */
export function recordedProfiles(host: HostProfileSlot): RecordedHostProfile[] {
  const folder = path.join(PROFILE_ROOT, host);
  let files: string[];
  try {
    files = readdirSync(folder).filter((file) => file.endsWith('.json'));
  } catch {
    return [];
  }
  return files
    .map((file) => JSON.parse(readFileSync(path.join(folder, file), 'utf8')) as RecordedHostProfile)
    .sort((left, right) => compareReleases(left.release, right.release));
}

/** The release SugarCubes is building now, which names a newly recorded profile. */
export function currentRelease(): string {
  return (JSON.parse(readFileSync('package.json', 'utf8')) as { version: string }).version;
}

/**
 * Require the newest recorded profile to match what the host does now.
 *
 * A host whose answers changed records a new profile for the current release and keeps the
 * older ones, so replay keeps proving the page against every recorded release.
 */
export function expectNewestProfile(recorded: RecordedHostProfile): void {
  const newest = recordedProfiles(recorded.host).at(-1);
  const same =
    newest !== undefined &&
    JSON.stringify(newest.transcript) === JSON.stringify(recorded.transcript);
  if (!same)
    throw new Error(
      `The ${recorded.host} host answers differently from its newest recorded profile. ` +
        `Save this as ${PROFILE_ROOT}/${recorded.host}/${recorded.release}.json:
` +
        JSON.stringify(recorded, null, 2),
    );
}
