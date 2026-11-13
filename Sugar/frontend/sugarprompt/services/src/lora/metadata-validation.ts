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
/** Shared LoRA metadata validation owns field rules for resource replies and immutable catalog input. */
import type { LoraCatalogItem, LoraModelIdentity, LoraThumbnail } from './contracts.js';
import { strip } from '../../../core/src/prompt/text.js';

/** Only this validator can brand immutable descriptors; reply adaptation retains actual catalog media identity. */
const validatedThumbnails = new WeakSet<object>();

/** Require exactly the expected own enumerable fields before copying unknown host data into plain frozen records. */
export function loraRecord(value: unknown, keys: readonly string[]): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value))
    throw new TypeError('Invalid LoRA resource record');
  const actual = Object.keys(value);
  if (actual.length !== keys.length || actual.some((key) => !keys.includes(key)))
    throw new TypeError('Invalid LoRA resource record');
  return value as Record<string, unknown>;
}

/** Bound literal host strings without normalizing source identities or display content. */
export function loraText(value: unknown, maximum: number, empty = false): string {
  if (typeof value !== 'string' || (!empty && value.length === 0) || value.length > maximum)
    throw new TypeError('Invalid LoRA resource text');
  return value;
}

/** Syntax rejection has a stable message; unexpected URL implementation errors remain observable. */
function parseUrl(value: string): URL {
  try {
    return new URL(value);
  } catch (cause) {
    if (cause instanceof TypeError) throw new TypeError('Invalid LoRA resource URL');
    throw cause;
  }
}

/** Acquired media additionally accepts HTTP(S)-origin blobs; host model pages accept only credential-free HTTP(S). */
function loraUrl(value: unknown, blob: boolean): string {
  const url = loraText(value, 4096),
    parsed = parseUrl(url);
  const target = parsed.protocol === 'blob:' && blob ? parseUrl(parsed.pathname) : parsed;
  if (
    (!blob && parsed.protocol === 'blob:') ||
    !['http:', 'https:', ...(blob ? ['blob:'] : [])].includes(parsed.protocol) ||
    !['http:', 'https:'].includes(target.protocol) ||
    !target.hostname ||
    target.username ||
    target.password
  )
    throw new TypeError('Invalid LoRA resource URL');
  return url;
}

/** Prepare the donor's trimmed page spelling through the credential-free HTTP(S) host boundary. */
export function readLoraModelPageUrl(value: unknown): string {
  return loraUrl(strip(loraText(value, 4096)), false);
}

/** Clone acquired image descriptors with bounded integral intrinsic dimensions, without acquiring or decoding media. */
function thumbnail(value: unknown): LoraThumbnail {
  if (value && typeof value === 'object' && validatedThumbnails.has(value))
    return value as LoraThumbnail;
  const raw = loraRecord(
    value,
    value && typeof value === 'object' && 'status' in value && value.status === 'ready'
      ? ['status', 'url', 'width', 'height']
      : ['status'],
  );
  if (raw.status === 'unavailable') return Object.freeze({ status: 'unavailable' });
  if (raw.status !== 'ready') throw new TypeError('Invalid LoRA thumbnail status');
  const url = loraUrl(raw.url, true);
  for (const size of [raw.width, raw.height])
    if (typeof size !== 'number' || !Number.isSafeInteger(size) || size < 1 || size > 8192)
      throw new TypeError('Invalid LoRA thumbnail dimensions');
  const prepared: LoraThumbnail = Object.freeze({
    status: 'ready',
    url,
    width: raw.width as number,
    height: raw.height as number,
  });
  validatedThumbnails.add(prepared);
  return prepared;
}

/** Copy canonical identities literally; lookup normalization is derived separately and never mutates these values. */
export function readLoraModelIdentity(value: unknown): LoraModelIdentity {
  const raw = loraRecord(value, ['promptName', 'backendValue', 'basename']);
  const basename = loraText(raw.basename, 4096);
  if (/[\\/]/u.test(basename)) throw new TypeError('Invalid LoRA basename');
  return Object.freeze({
    promptName: loraText(raw.promptName, 4096),
    backendValue: loraText(raw.backendValue, 4096),
    basename,
  });
}

/** Catalog and found-resource boundaries share one metadata owner after their outer closed schemas are checked. */
export function readLoraItemFields(raw: Record<string, unknown>): LoraCatalogItem {
  if (!Array.isArray(raw.triggerWords) || raw.triggerWords.length > 256)
    throw new TypeError('Invalid LoRA trigger words');
  return Object.freeze({
    model: readLoraModelIdentity(raw.model),
    title: loraText(raw.title, 256, true),
    subtitle: raw.subtitle === null ? null : loraText(raw.subtitle, 256, true),
    triggerWords: Object.freeze(raw.triggerWords.map((word: unknown) => loraText(word, 1024))),
    thumbnail: thumbnail(raw.thumbnail),
    modelPageUrl: raw.modelPageUrl === null ? null : readLoraModelPageUrl(raw.modelPageUrl),
  });
}

/** Both metadata boundaries share this closed shape; the reply boundary adds requested identity and status. */
export const loraItemKeys = Object.freeze([
  'model',
  'title',
  'subtitle',
  'triggerWords',
  'thumbnail',
  'modelPageUrl',
]);
