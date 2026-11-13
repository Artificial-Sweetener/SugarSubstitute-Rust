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
import type { LoraResourceAdapter } from '../../../sugarprompt/services/src/index.js';
import type { ModelCatalogSnapshot, ModelCatalogItem } from '../model_library/ModelCatalog.js';

/** Preserve installed identities and prefer the model's authored banner artwork. */
export function preparePromptLoraCatalog(
  snapshot: ModelCatalogSnapshot,
  baseUrl: string,
  prioritize?: (items: readonly ModelCatalogItem[]) => void,
): LoraResourceAdapter {
  const models = new Map(
    snapshot.items.filter((item) => item.kind === 'loras').map((item) => [item.value, item]),
  );
  const adapter = createLoraCatalogAdapter({
    revision: `models:${String(snapshot.revision)}`,
    authoritative: !snapshot.scanning,
    items: snapshot.items
      .filter((item) => item.kind === 'loras')
      .map((item) => {
        const name = item.value.replace(/\.(safetensors|ckpt|pt|pth|bin)$/i, '');
        const thumbnail =
          item.thumbnails.find((image) => image.role === 'banner') ?? item.thumbnails[0];
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
      }),
  });
  return {
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
      return resources;
    },
  };
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
