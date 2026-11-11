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
/** Validate/copy one finite output-choice capability outside native input and rendering. */
import { loraRecord, loraText } from './metadata-validation.js';
import type {
  LoraMetadataAdapter,
  LoraMetadataResult,
  LoraOutputImage,
} from './metadata-actions-contracts.js';

/** Copied capabilities capture the actual host method and immutable prepared choices. */
export type PreparedLoraMetadata = LoraMetadataAdapter;

const capabilities = new WeakSet<PreparedLoraMetadata>();

/** Bound integral image grouping and optional intrinsic dimensions before menu preparation. */
function integer(value: unknown, minimum: number, maximum: number): number {
  if (
    typeof value !== 'number' ||
    !Number.isSafeInteger(value) ||
    value < minimum ||
    value > maximum
  )
    throw new TypeError('Invalid output-image integer');
  return value;
}

/** A host-provided final-output image has a canonical UUID and literal bounded scene/source grouping. */
function image(value: unknown): LoraOutputImage {
  const raw = loraRecord(value, [
    'imageId',
    'sceneKey',
    'sceneTitle',
    'sceneOrder',
    'sourceKey',
    'sourceLabel',
    'setIndex',
    'width',
    'height',
  ]);
  const imageId = loraText(raw.imageId, 36);
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/iu.test(imageId))
    throw new TypeError('Invalid output-image identity');
  return Object.freeze({
    imageId: imageId.toLowerCase(),
    sceneKey: loraText(raw.sceneKey, 256, true),
    sceneTitle: loraText(raw.sceneTitle, 512, true),
    sceneOrder: integer(raw.sceneOrder, 0, 1_000_000),
    sourceKey: loraText(raw.sourceKey, 256),
    sourceLabel: loraText(raw.sourceLabel, 512, true),
    setIndex: integer(raw.setIndex, 0, 1_000_000),
    width: raw.width === null ? null : integer(raw.width, 1, 16384),
    height: raw.height === null ? null : integer(raw.height, 1, 16384),
  });
}

/** Deduplicate final image identities in donor display order and require a current active choice. */
export function prepareLoraMetadata(adapter: LoraMetadataAdapter): PreparedLoraMetadata {
  const raw = loraRecord(adapter, [
    'revision',
    'refresh',
    'thumbnail',
    'choices',
    'activeImageId',
    'execute',
  ]);
  if (
    typeof raw.refresh !== 'boolean' ||
    typeof raw.thumbnail !== 'boolean' ||
    typeof raw.execute !== 'function' ||
    !Array.isArray(raw.choices) ||
    raw.choices.length > 512
  )
    throw new TypeError('Invalid LoRA metadata capability');
  const choices = new Map<string, LoraOutputImage>();
  for (const value of raw.choices) {
    const prepared = image(value);
    if (!choices.has(prepared.imageId)) choices.set(prepared.imageId, prepared);
  }
  const active = raw.activeImageId === null ? null : loraText(raw.activeImageId, 36).toLowerCase();
  if (active !== null && !choices.has(active)) throw new TypeError('Invalid active output image');
  // The callable cast is confined to this validated external capability boundary.
  const execute = raw.execute as (
    request: Parameters<LoraMetadataAdapter['execute']>[0],
    signal: AbortSignal,
  ) => Promise<LoraMetadataResult>;
  const prepared: PreparedLoraMetadata = Object.freeze({
    revision: loraText(raw.revision, 256),
    refresh: raw.refresh,
    thumbnail: raw.thumbnail,
    choices: Object.freeze([...choices.values()]),
    activeImageId: active,
    execute: execute.bind(adapter),
  });
  capabilities.add(prepared);
  return prepared;
}

/** Only copied prepared capabilities may create or execute native action requests. */
export function isPreparedLoraMetadata(value: unknown): value is PreparedLoraMetadata {
  return (
    typeof value === 'object' && value !== null && capabilities.has(value as PreparedLoraMetadata)
  );
}

/** Admit optional mounting absence or the exact service-prepared authority, without creating a request lifetime. */
export function validatePreparedLoraMetadata(
  value: unknown,
): asserts value is PreparedLoraMetadata | undefined {
  if (value !== undefined && !isPreparedLoraMetadata(value))
    throw new TypeError('Invalid prepared metadata capability');
}
