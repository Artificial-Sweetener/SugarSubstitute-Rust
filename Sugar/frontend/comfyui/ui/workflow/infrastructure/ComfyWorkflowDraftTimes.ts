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
/** Read when Comfy last kept its own copy of each open workflow, from the index it stores in the page. */
import { isRecord } from '../../types/common.js';

/** Comfy's draft index, one per Comfy workspace; any other version is a format this reader does not know. */
const DRAFT_INDEX = 'Comfy.Workflow.DraftIndex.';
const KNOWN_DRAFT_INDEX = `${DRAFT_INDEX}v2:`;

/** When Comfy kept a workflow: a time, `null` for no copy, or `unreadable` when the index cannot be trusted. */
export type ComfyDraftTime = number | null | 'unreadable';

/** Answer for one workflow path. */
export type ComfyDraftTimes = (path: string) => ComfyDraftTime;

/**
 * Read every draft time once, now.
 *
 * Comfy rewrites the draft of the workflow it shows as soon as it restores its tabs, so the times
 * that describe what the last session left behind exist only before that restoration.
 */
export function readComfyDraftTimes(
  storage: Pick<Storage, 'length' | 'key' | 'getItem'>,
): ComfyDraftTimes {
  const times = new Map<string, number>();
  let readable = true;
  try {
    for (let index = 0; index < storage.length; index += 1) {
      const key = storage.key(index);
      if (!key?.startsWith(DRAFT_INDEX)) continue;
      const entries = key.startsWith(KNOWN_DRAFT_INDEX) ? draftEntries(storage.getItem(key)) : null;
      if (!entries) {
        readable = false;
        break;
      }
      for (const [path, at] of entries) times.set(path, Math.max(times.get(path) ?? at, at));
    }
  } catch {
    // Storage the page may not read is the same as an index that cannot be trusted.
    readable = false;
  }
  return (path) => (readable ? (times.get(path) ?? null) : 'unreadable');
}

/** Validate one stored index into its path and time pairs; `null` when it is not the known shape. */
function draftEntries(stored: string | null): readonly (readonly [string, number])[] | null {
  if (stored === null) return null;
  let index: unknown;
  try {
    index = JSON.parse(stored);
  } catch {
    return null;
  }
  if (!isRecord(index) || index.v !== 2 || !isRecord(index.entries)) return null;
  const entries: (readonly [string, number])[] = [];
  for (const entry of Object.values(index.entries)) {
    if (
      !isRecord(entry) ||
      typeof entry.path !== 'string' ||
      typeof entry.updatedAt !== 'number' ||
      !Number.isFinite(entry.updatedAt)
    )
      return null;
    entries.push([entry.path, entry.updatedAt]);
  }
  return entries;
}
