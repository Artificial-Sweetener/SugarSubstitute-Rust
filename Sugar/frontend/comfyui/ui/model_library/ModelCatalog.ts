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
/** Validate the shared model-library catalog before presenting backend values. */
import { isRecord } from '../types/common.js';
import { decodeModelTraits, type ModelTraits } from './ModelTraits.js';
import { decodeModelRelationships, type ModelRelationshipCatalog } from './ModelRelationships.js';

export interface ModelThumbnail {
  readonly assetId: string;
  readonly role: 'standard' | 'banner';
  readonly width: number;
  readonly height: number;
  readonly url: string;
}

/**
 * What a model's card draws where its image goes.
 *
 * - `ready`: an image.
 * - `pending`: a lookup or an image fetch is queued or running.
 * - `retrying`: the same, after a failure.
 * - `filtered`: an image exists that the thumbnail safety setting excludes.
 * - `not_found`: no provider knows the file.
 * - `none`: the model is known, or is never looked up, and has no image.
 */
export type ModelImageState = 'ready' | 'pending' | 'retrying' | 'filtered' | 'not_found' | 'none';

export interface ModelCardImage {
  readonly state: ModelImageState;
  /** Whether the model's thumbnails are blurred files. */
  readonly blurred: boolean;
  /** The sharp files of a blurred image, present only when hover reveal is switched on. */
  readonly reveal: readonly ModelThumbnail[];
}

export interface ModelCatalogItem {
  readonly image?: ModelCardImage;
  readonly versionCount?: number | null;
  readonly architecture?: ModelArchitecture | null;
  readonly traits?: ModelTraits | null;
  readonly detectorAsset?: string;
  readonly id: string;
  readonly kind: string;
  readonly value: string;
  readonly rootId: string;
  readonly relativePath: string;
  readonly folder: string;
  readonly title: string;
  readonly version: string;
  readonly baseModel: string;
  readonly provider: string;
  readonly providerUrl: string;
  readonly providerModelId: string;
  readonly providerVersionId: string;
  readonly providerLinks: readonly ModelProviderLink[];
  readonly description: string;
  readonly trainedWords: readonly string[];
  readonly tags?: readonly string[];
  readonly thumbnails: readonly ModelThumbnail[];
}

/** Keep host tensor architecture distinct from provider family labels. */
export interface ModelArchitecture {
  readonly base: string;
  readonly comfyClass: string;
  readonly source: 'comfy_tensor_shapes';
}

export interface ModelProviderLink {
  readonly providerId: string;
  readonly providerName: string;
  readonly modelId: string;
  readonly versionId: string;
  readonly modelPageUrl: string;
}

export interface ModelCatalogSnapshot {
  readonly relationships?: ModelRelationshipCatalog | null;
  readonly revision: number;
  readonly items: readonly ModelCatalogItem[];
  readonly scanning: boolean;
  readonly fieldContracts: readonly ModelFieldContract[];
}

export interface ModelFieldContract {
  readonly classType: string;
  readonly inputName: string;
  readonly kind: string;
}

/** Reject unknown schemas and invalid image addresses at the browser boundary. */
export function decodeModelCatalog(value: unknown): ModelCatalogSnapshot {
  if (
    !isRecord(value) ||
    value.schemaVersion !== 1 ||
    !Array.isArray(value.models) ||
    !Array.isArray(value.fieldContracts)
  )
    throw new Error('Unsupported model catalog response.');
  if (
    typeof value.revision !== 'number' ||
    !Number.isSafeInteger(value.revision) ||
    value.revision < 0
  )
    throw new Error('Invalid model catalog revision.');
  const scan = isRecord(value.scan) ? value.scan : {};
  return {
    revision: value.revision,
    relationships: decodeModelRelationships(value.relationships),
    items: value.models.map(decodeItem),
    scanning: scan.status === 'queued' || scan.status === 'running',
    fieldContracts: value.fieldContracts.map((contract: unknown) => {
      if (!isRecord(contract)) throw new Error('Invalid model field contract.');
      return {
        classType: text(contract.classType),
        inputName: text(contract.inputName),
        kind: text(contract.kind),
      };
    }),
  };
}

/** Preserve literal text and exact Comfy choices instead of coercing dynamic data. */
function decodeItem(value: unknown): ModelCatalogItem {
  if (!isRecord(value) || !Array.isArray(value.thumbnails) || !Array.isArray(value.trainedWords))
    throw new Error('Invalid model catalog item.');
  return {
    ...(value.detectorAsset === undefined ? {} : { detectorAsset: text(value.detectorAsset) }),
    id: text(value.id),
    kind: text(value.kind),
    value: text(value.value),
    rootId: text(value.rootId),
    relativePath: text(value.relativePath),
    folder: text(value.folder),
    title: text(value.title),
    version: text(value.version),
    baseModel: text(value.baseModel),
    versionCount: decodeVersionCount(value.versionCount),
    architecture: decodeArchitecture(value.architecture),
    traits: decodeModelTraits(value.traits),
    provider: text(value.provider),
    providerUrl: text(value.providerUrl),
    providerModelId: value.providerModelId === undefined ? '' : text(value.providerModelId),
    providerVersionId: value.providerVersionId === undefined ? '' : text(value.providerVersionId),
    providerLinks: decodeProviderLinks(value.providerLinks),
    description: text(value.description),
    trainedWords: value.trainedWords.map(text),
    tags: value.tags === undefined ? [] : decodeTags(value.tags),
    thumbnails: value.thumbnails.map(decodeThumbnail),
    image: decodeCardImage(value.image, value.thumbnails.length > 0),
  };
}

const IMAGE_STATES: readonly ModelImageState[] = [
  'ready',
  'pending',
  'retrying',
  'filtered',
  'not_found',
  'none',
];

/** Accept catalogs without card states, reading them as an image or as no image. */
function decodeCardImage(value: unknown, pictured: boolean): ModelCardImage {
  if (value === undefined)
    return { state: pictured ? 'ready' : 'none', blurred: false, reveal: [] };
  if (
    !isRecord(value) ||
    typeof value.blurred !== 'boolean' ||
    !Array.isArray(value.reveal) ||
    !IMAGE_STATES.some((state) => state === value.state)
  )
    throw new Error('Invalid model card image.');
  return {
    state: value.state as ModelImageState,
    blurred: value.blurred,
    reveal: value.reveal.map(decodeThumbnail),
  };
}

/** The models a published change names, and the ones it removed. */
export interface ModelCatalogChanges {
  readonly revision: number;
  readonly items: readonly ModelCatalogItem[];
  readonly removed: readonly string[];
}

/** The server's answer to "what changed since the revision I hold". */
export interface ModelCatalogUpdate {
  readonly revision: number;
  /** The changed models, or null when the whole catalog must be read again. */
  readonly changes: ModelCatalogChanges | null;
}

/** Validate a change answer before any of it reaches the held catalog. */
export function decodeModelCatalogUpdate(value: unknown): ModelCatalogUpdate {
  if (
    !isRecord(value) ||
    value.schemaVersion !== 1 ||
    typeof value.revision !== 'number' ||
    !Number.isSafeInteger(value.revision) ||
    value.revision < 0
  )
    throw new Error('Invalid model catalog update response.');
  if (value.models === undefined) return { revision: value.revision, changes: null };
  if (!Array.isArray(value.models) || !Array.isArray(value.removed))
    throw new Error('Invalid model catalog changes.');
  return {
    revision: value.revision,
    changes: {
      revision: value.revision,
      items: value.models.map(decodeItem),
      removed: value.removed.map(text),
    },
  };
}

/**
 * Apply the models a change names to the catalog it follows. Every other entry stays the same
 * object, so consumers redo nothing for them. A change at or before the held revision is
 * already applied and returns the held catalog itself.
 */
export function applyModelCatalogChanges(
  held: ModelCatalogSnapshot,
  changes: ModelCatalogChanges,
): ModelCatalogSnapshot {
  if (changes.revision <= held.revision) return held;
  const removed = new Set(changes.removed);
  const changed = new Map(changes.items.map((item) => [item.id, item]));
  const items = held.items
    .filter((item) => !removed.has(item.id))
    .map((item) => {
      const next = changed.get(item.id);
      changed.delete(item.id);
      return next ?? item;
    });
  return continueModelCatalog(held, {
    ...held,
    revision: changes.revision,
    items: [...items, ...changed.values()],
  });
}

/** Validate additive tensor evidence while preserving older public catalog payloads. */
function decodeArchitecture(value: unknown): ModelArchitecture | null {
  if (value === undefined || value === null) return null;
  if (!isRecord(value) || value.source !== 'comfy_tensor_shapes')
    throw new Error('Invalid model architecture evidence.');
  const base = text(value.base);
  const comfyClass = text(value.comfyClass);
  if (!base.trim() || !comfyClass.trim()) throw new Error('Invalid model architecture label.');
  return { base, comfyClass, source: 'comfy_tensor_shapes' };
}

/** Preserve unknown history while rejecting malformed prepared version counts. */
function decodeVersionCount(value: unknown): number | null {
  if (value === undefined || value === null) return null;
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0)
    throw new Error('Invalid model version count.');
  return value;
}

/** Accept older catalogs while validating additive family-discovery metadata. */
function decodeTags(value: unknown): readonly string[] {
  if (!Array.isArray(value)) throw new Error('Invalid model tags.');
  return value.map(text);
}

/** Accept the additive public schema while rejecting malformed association arrays. */
function decodeProviderLinks(value: unknown): readonly ModelProviderLink[] {
  if (value === undefined) return [];
  if (!Array.isArray(value) || value.length > 16) throw new Error('Invalid model provider links.');
  return value.map((item: unknown) => {
    if (!isRecord(item)) throw new Error('Invalid model provider link.');
    return {
      providerId: text(item.providerId),
      providerName: text(item.providerName),
      modelId: text(item.modelId),
      versionId: text(item.versionId),
      modelPageUrl: text(item.modelPageUrl),
    };
  });
}

/** Admit only the library's immutable image endpoint and known tile geometry. */
function decodeThumbnail(value: unknown): ModelThumbnail {
  if (!isRecord(value) || (value.role !== 'standard' && value.role !== 'banner'))
    throw new Error('Invalid model thumbnail reference.');
  const assetId = text(value.assetId);
  const width = value.width;
  const height = value.height;
  if (
    !/^[a-f0-9]{64}$/.test(assetId) ||
    typeof width !== 'number' ||
    typeof height !== 'number' ||
    !Number.isSafeInteger(width) ||
    !Number.isSafeInteger(height) ||
    width <= 0 ||
    height <= 0
  )
    throw new Error('Invalid model thumbnail dimensions or identity.');
  const url = `/sugarcubes/v1/models/assets/${assetId}`;
  if (value.url !== url) throw new Error('Invalid model thumbnail URL.');
  return { assetId, role: value.role, width, height, url };
}

/** Narrow untrusted labels without treating their contents as markup. */
function text(value: unknown): string {
  if (typeof value !== 'string') throw new Error('Expected model catalog text.');
  return value;
}

/**
 * Carry everything a new catalog generation left unchanged over from the one it replaces.
 *
 * The library republishes the whole catalog for a change to any one model. Every consumer reads
 * this one snapshot, so an entry that did not change stays the same object, and `items` stays the
 * same array when no entry changed: consumers compare identities and redo nothing for the rest.
 * The revision always advances, because the change subscription follows it.
 */
export function continueModelCatalog(
  previous: ModelCatalogSnapshot | null,
  next: ModelCatalogSnapshot,
): ModelCatalogSnapshot {
  if (!previous) return next;
  const same = (before: unknown, after: unknown): boolean =>
    JSON.stringify(before) === JSON.stringify(after);
  const known = new Map(previous.items.map((item) => [item.id, item]));
  let unchanged = previous.items.length === next.items.length;
  const items = next.items.map((item, index) => {
    const before = known.get(item.id);
    const kept = before && same(before, item) ? before : item;
    if (kept !== previous.items[index]) unchanged = false;
    return kept;
  });
  return {
    ...next,
    items: unchanged ? previous.items : items,
    fieldContracts: same(previous.fieldContracts, next.fieldContracts)
      ? previous.fieldContracts
      : next.fieldContracts,
    ...(previous.relationships !== undefined &&
    same(previous.relationships, next.relationships ?? null)
      ? { relationships: previous.relationships }
      : {}),
  };
}

/**
 * A list of models that follows the catalog.
 *
 * A surface that shows models while it is open takes one of these, never an array: an array is
 * the catalog as it was when the surface opened, and a surface holding one keeps showing a model
 * as it was after its image or its labels have changed.
 */
export interface LiveModels {
  /** The models as the catalog has them now. */
  current(): readonly ModelCatalogItem[];
  /** Be told whenever `current` changes, until the returned release is called. */
  watch(changed: () => void): () => void;
}

/** The size a model's banner is asked for, by a field and by a prompt's LoRA chip alike. */
export const MODEL_BANNER_SIZE = 768;

/**
 * The one rule for which of a model's prepared images a surface shows: the smallest of the wanted
 * role that is large enough for its destination, without decoding originals.
 */
export function chooseModelThumbnail(
  item: ModelCatalogItem,
  role: 'standard' | 'banner',
  size: number,
): ModelThumbnail | null {
  const candidates = item.thumbnails
    .filter((image) => image.role === role)
    .sort((left, right) => Math.max(left.width, left.height) - Math.max(right.width, right.height));
  return (
    candidates.find((image) => Math.max(image.width, image.height) >= size) ??
    candidates.at(-1) ??
    null
  );
}

/** Whether two snapshots list the same model files in the same state of discovery. */
export function sameModelFiles(left: ModelCatalogSnapshot, right: ModelCatalogSnapshot): boolean {
  return (
    left.scanning === right.scanning &&
    left.items.length === right.items.length &&
    left.items.every((item, index) => {
      const other = right.items[index];
      return (
        item === other ||
        (other !== undefined &&
          item.id === other.id &&
          item.kind === other.kind &&
          item.value === other.value)
      );
    })
  );
}
