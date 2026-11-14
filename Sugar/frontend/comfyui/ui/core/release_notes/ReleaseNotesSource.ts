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
/**
 * Describe where the What's new view reads one product's published release notes.
 *
 * A host that publishes notes of its own feeds them through the `sugarcubes.release-notes`
 * protocol; SugarCubes reads its own from its backend. Both implement {@link ReleaseNotesSource},
 * so one view shows every product's notes the same way.
 */
import type { ReleaseNotesFailure } from '../embedded/ReleaseNotesRuntime.js';

export type { ReleaseNotesFailure } from '../embedded/ReleaseNotesRuntime.js';

/** One version's notes, as the Markdown its changelog holds. */
export interface PublishedRelease {
  readonly version: string;
  readonly notes: string;
}

/** The answer to one notes read; a read never rejects. */
export type ReleaseNotesOutcome =
  | {
      readonly kind: 'notes';
      /** Newest version first. */
      readonly releases: readonly PublishedRelease[];
      /** Older versions in the range left out to keep the answer bounded. */
      readonly omitted: number;
    }
  | { readonly kind: 'failed'; readonly reason: ReleaseNotesFailure };

/** The product a source speaks for and the version it runs. */
export interface ReleaseNotesIdentity {
  readonly product: string;
  readonly currentVersion: string;
}

/** One product's published release notes. */
export interface ReleaseNotesSource {
  /** The product and running version, or null until the source knows them. */
  read(): ReleaseNotesIdentity | null;
  subscribe(changed: () => void): () => void;
  /**
   * Read the notes of every version after `since` through the running version; a `since` that
   * is not older than the running version reads the running version alone.
   */
  load(since: string): Promise<ReleaseNotesOutcome>;
}

const UNAVAILABLE: ReleaseNotesOutcome = Object.freeze({ kind: 'failed', reason: 'unavailable' });

/** The outcome every source reports when it cannot read notes now. */
export function unavailableNotes(): ReleaseNotesOutcome {
  return UNAVAILABLE;
}

const RELEASE = /^(\d{1,9})\.(\d{1,9})\.(\d{1,9})(?:-([a-z0-9.-]+))?$/;

/**
 * Order two release tokens: numeric `major.minor.patch`, a release after its own prereleases.
 *
 * @returns A negative number, zero or a positive number, or null when either is not a version.
 */
export function compareReleaseVersions(left: string, right: string): number | null {
  const a = RELEASE.exec(left);
  const b = RELEASE.exec(right);
  if (!a || !b) return null;
  for (const index of [1, 2, 3]) {
    const difference = Number(a[index]) - Number(b[index]);
    if (difference !== 0) return difference;
  }
  const [preA, preB] = [a[4], b[4]];
  if (preA === preB) return 0;
  if (preA === undefined) return 1;
  if (preB === undefined) return -1;
  return preA < preB ? -1 : 1;
}
