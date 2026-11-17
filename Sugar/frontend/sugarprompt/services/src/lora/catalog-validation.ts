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
/** Turn a host catalog into immutable authoritative data before lookup indexing. */
import type { LoraCatalogSnapshot, LoraThumbnail } from './contracts.js';
import { loraItemKeys, loraRecord, loraText, readLoraItemFields } from './metadata-validation.js';

/** Name media by what it shows: its address and intrinsic size. */
function mediaKey(image: Extract<LoraThumbnail, { status: 'ready' }>): string {
  return JSON.stringify([image.url, image.width, image.height]);
}

/** Reject malformed catalogs and duplicate backend identities; normalized aliases may remain legitimately ambiguous.
 * Media that `continued`, the validated catalog this one replaces, already described keeps that
 * catalog's descriptor, so pixels prepared for it stay valid. Changed media gets a new descriptor.
 */
export function readLoraCatalog(
  value: unknown,
  continued?: LoraCatalogSnapshot,
): LoraCatalogSnapshot {
  const raw = loraRecord(value, ['revision', 'authoritative', 'items']);
  if (
    typeof raw.authoritative !== 'boolean' ||
    !Array.isArray(raw.items) ||
    raw.items.length > 100_000
  )
    throw new TypeError('Invalid LoRA catalog');
  const physical = new Set<string>();
  const thumbnails = new Map<string, LoraThumbnail>();
  for (const item of continued?.items ?? [])
    if (item.thumbnail.status === 'ready') thumbnails.set(mediaKey(item.thumbnail), item.thumbnail);
  const items = raw.items.map((item: unknown) => {
    const prepared = readLoraItemFields(loraRecord(item, loraItemKeys));
    if (physical.has(prepared.model.backendValue))
      throw new TypeError('Duplicate LoRA backend identity');
    physical.add(prepared.model.backendValue);
    if (prepared.thumbnail.status === 'ready') {
      const image = prepared.thumbnail;
      const key = mediaKey(image);
      const existing = thumbnails.get(key);
      if (existing) return Object.freeze({ ...prepared, thumbnail: existing });
      thumbnails.set(key, image);
    }
    return prepared;
  });
  return Object.freeze({
    revision: loraText(raw.revision, 256),
    authoritative: raw.authoritative,
    items: Object.freeze(items),
  });
}
