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
/** Adapt the shared model library into SugarPrompt's canonical LoRA and thumbnail catalog. */
import { createLoraCatalogAdapter } from '../../../sugarprompt/services/src/index.js';
import type {
  LoraCatalogItem,
  LoraResourceAdapter,
} from '../../../sugarprompt/services/src/index.js';
import type { ModelCatalogSnapshot, ModelCatalogItem } from '../model_library/ModelCatalog.js';
import { MODEL_BANNER_SIZE, chooseModelThumbnail } from '../model_library/ModelCatalog.js';

/**
 * The installed LoRAs each prepared catalog was made from, and the adapter that resolves them.
 * A catalog is replaced when what a prompt can write or search for changes. Images alone change
 * in place: `models` and `items` then describe the newest images and `media` counts the change.
 */
interface PreparedCatalog {
  readonly adapter: LoraResourceAdapter;
  readonly authoritative: boolean;
  loras: readonly ModelCatalogItem[];
  readonly models: Map<string, ModelCatalogItem>;
  readonly items: Map<string, LoraCatalogItem>;
  media: number;
}
const prepared = new WeakMap<LoraResourceAdapter, PreparedCatalog>();

/** The name a prompt writes for an installed LoRA: its path without the model file extension. */
export function promptLoraName(backendValue: string): string {
  return backendValue.replace(/\.(safetensors|ckpt|pt|pth|bin)$/i, '');
}

/** The editor's record of one installed LoRA in a prepared catalog, by its backend value. */
export function promptLoraItem(
  catalog: LoraResourceAdapter,
  backendValue: string,
): LoraCatalogItem | null {
  return prepared.get(catalog)?.items.get(backendValue) ?? null;
}

/** The installed model a prepared catalog made one LoRA from, by its backend value. */
export function promptLoraModel(
  catalog: LoraResourceAdapter,
  backendValue: string,
): ModelCatalogItem | null {
  return prepared.get(catalog)?.models.get(backendValue) ?? null;
}

/** How many times a prepared catalog's images have changed in place. */
export function promptLoraMediaRevision(catalog: LoraResourceAdapter): number {
  return prepared.get(catalog)?.media ?? 0;
}

/**
 * Preserve installed identities and prefer the model's authored banner artwork.
 *
 * The model library publishes a new snapshot for every change to any model, and its catalog
 * keeps every unchanged model as the same object. Pass the catalog being replaced as `previous`:
 * a snapshot whose LoRAs are all those same objects returns that same catalog, so no editor is
 * disturbed. So does one in which only images changed: the catalog then carries the new images
 * and a higher `promptLoraMediaRevision`, and nothing a prompt has prepared is retired. Any other
 * change returns a new catalog that keeps the descriptor of every unchanged image, so only
 * changed images are decoded again.
 */
export function preparePromptLoraCatalog(
  snapshot: ModelCatalogSnapshot,
  baseUrl: string,
  prioritize?: (items: readonly ModelCatalogItem[]) => void,
  previous?: LoraResourceAdapter,
): LoraResourceAdapter {
  const loras = snapshot.items.filter((item) => item.kind === 'loras');
  const authoritative = !snapshot.scanning;
  const replaced = previous ? prepared.get(previous) : undefined;
  if (
    previous &&
    replaced?.authoritative === authoritative &&
    replaced.loras.length === loras.length &&
    replaced.loras.every((item, index) => item === loras[index])
  )
    return previous;
  if (
    previous &&
    replaced?.authoritative === authoritative &&
    replaced.loras.length === loras.length &&
    replaced.loras.every((item, index) => sameWords(item, loras[index]))
  ) {
    replaced.loras.forEach((item, index) => {
      const next = loras[index];
      if (!next || next === item) return;
      const held = replaced.items.get(next.value)?.thumbnail;
      const made = promptLoraDescription(next, baseUrl);
      replaced.models.set(next.value, next);
      replaced.items.set(
        next.value,
        held && sameImage(held, made.thumbnail) ? { ...made, thumbnail: held } : made,
      );
    });
    replaced.loras = loras;
    replaced.media++;
    return previous;
  }
  const models = new Map(loras.map((item) => [item.value, item]));
  // An unchanged model is described by the same object as before, which is what lets the
  // adapter keep everything it derived from it.
  const items = loras.map((item) =>
    replaced?.models.get(item.value) === item
      ? (replaced.items.get(item.value) ?? promptLoraDescription(item, baseUrl))
      : promptLoraDescription(item, baseUrl),
  );
  const adapter = createLoraCatalogAdapter(
    { revision: `models:${String(snapshot.revision)}`, authoritative, items },
    // Everything a person may know a LoRA by finds it when they type its name.
    {
      searchMetadata: loras.map((item) => ({
        backendValue: item.value,
        text: [
          item.title,
          item.version,
          item.value,
          item.relativePath,
          item.folder,
          item.baseModel,
          ...item.trainedWords,
          ...(item.tags ?? []),
        ]
          .filter(Boolean)
          .join(' '),
      })),
    },
    replaced?.adapter,
  );
  const catalog: LoraResourceAdapter = {
    ...adapter,
    /** Prioritize only names resolved by the native catalog; never acquire metadata from rendering. */
    async resolve(names, signal) {
      const resources = await adapter.resolve(names, signal);
      if (!signal.aborted) {
        const items = new Set<ModelCatalogItem>();
        for (const resource of resources) {
          if (resource.resolution !== 'found') continue;
          const model = models.get(resource.model.backendValue);
          if (model) items.add(model);
        }
        prioritize?.([...items]);
      }
      // Images that changed since this catalog was indexed are the ones a chip shows.
      return record.media === 0
        ? resources
        : resources.map((resource) => {
            const image =
              resource.resolution === 'found'
                ? record.items.get(resource.model.backendValue)?.thumbnail
                : undefined;
            return resource.resolution === 'found' && image && image !== resource.thumbnail
              ? Object.freeze({ ...resource, thumbnail: image })
              : resource;
          });
    },
  };
  const record: PreparedCatalog = {
    adapter,
    authoritative,
    loras,
    models,
    items: new Map(items.map((item) => [item.model.backendValue, item])),
    media: 0,
  };
  prepared.set(catalog, record);
  return catalog;
}

/** Describe one installed LoRA to the editor, preferring its authored banner artwork. */
function promptLoraDescription(item: ModelCatalogItem, baseUrl: string): LoraCatalogItem {
  const name = promptLoraName(item.value);
  const thumbnail =
    chooseModelThumbnail(item, 'banner', MODEL_BANNER_SIZE) ??
    chooseModelThumbnail(item, 'standard', MODEL_BANNER_SIZE);
  const url = thumbnail ? safeHttpUrl(thumbnail.url, baseUrl) : null;
  return {
    model: {
      promptName: name,
      backendValue: item.value,
      basename: name.split(/[\\/]/).pop() ?? name,
    },
    title: item.title || item.value,
    subtitle: item.version || null,
    triggerWords: item.trainedWords,
    thumbnail:
      thumbnail && url && thumbnail.width > 0 && thumbnail.height > 0
        ? { status: 'ready' as const, url, width: thumbnail.width, height: thumbnail.height }
        : { status: 'unavailable' as const },
    modelPageUrl: modelPage(item, baseUrl),
  };
}

/** Whether two images are the same picture: the same address at the same size. */
function sameImage(
  left: LoraCatalogItem['thumbnail'],
  right: LoraCatalogItem['thumbnail'],
): boolean {
  return (
    left.status === 'ready' &&
    right.status === 'ready' &&
    left.url === right.url &&
    left.width === right.width &&
    left.height === right.height
  );
}

/** Whether two records of a model give a prompt the same names, words and links to work with. */
function sameWords(left: ModelCatalogItem, right: ModelCatalogItem | undefined): boolean {
  if (!right) return false;
  if (left === right) return true;
  const same = (a: readonly string[] | undefined, b: readonly string[] | undefined): boolean =>
    (a ?? []).length === (b ?? []).length && (a ?? []).every((word, index) => word === b?.[index]);
  return (
    left.value === right.value &&
    left.title === right.title &&
    left.version === right.version &&
    left.relativePath === right.relativePath &&
    left.folder === right.folder &&
    left.baseModel === right.baseModel &&
    left.providerUrl === right.providerUrl &&
    left.providerLinks[0]?.modelPageUrl === right.providerLinks[0]?.modelPageUrl &&
    same(left.trainedWords, right.trainedWords) &&
    same(left.tags, right.tags)
  );
}

/** Admit only browser-safe media and model pages from validated catalog addresses. */
function safeHttpUrl(value: string, baseUrl: string): string | null {
  if (!value) return null;
  try {
    const url = new URL(value, baseUrl);
    return url.protocol === 'https:' || url.protocol === 'http:' ? url.href : null;
  } catch {
    return null;
  }
}

/** Keep provider metadata optional without fabricating a page from an installed path. */
function modelPage(item: ModelCatalogItem, baseUrl: string): string | null {
  return safeHttpUrl(item.providerLinks[0]?.modelPageUrl ?? item.providerUrl, baseUrl);
}
