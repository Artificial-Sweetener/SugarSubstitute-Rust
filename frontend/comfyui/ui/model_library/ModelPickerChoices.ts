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
/** Resolve exact Comfy dropdown choices into searchable model presentation. */
import type { ModelCatalogItem } from './ModelCatalog.js';

/** Retain every host option, including sentinels and files without provider metadata. */
export function modelPickerChoices(
  items: readonly ModelCatalogItem[],
  kind: string,
  options: readonly string[],
): readonly ModelCatalogItem[] {
  const available = new Map(
    items.filter((item) => item.kind === kind).map((item) => [item.value, item]),
  );
  return options.map((value) => available.get(value) ?? literalModelChoice(kind, value));
}

/** Construct an honest filename-only entry until prepared metadata becomes available. */
function literalModelChoice(kind: string, value: string): ModelCatalogItem {
  const path = value.replaceAll('\\', '/');
  const filename = path.split('/').at(-1) ?? path;
  return {
    id: `literal:${kind}:${value}`,
    kind,
    value,
    rootId: '',
    relativePath: path,
    folder: path.includes('/') ? path.slice(0, path.lastIndexOf('/')) : '',
    title: filename.replace(/\.(?:safetensors|ckpt|pt|pth|bin|gguf)$/i, ''),
    version: '',
    baseModel: '',
    provider: '',
    providerUrl: '',
    providerModelId: '',
    providerVersionId: '',
    providerLinks: [],
    description: '',
    trainedWords: [],
    thumbnails: [],
  };
}

/** Filter within the active folder using all of Substitute's useful searchable labels. */
export function filterModelChoices(
  items: readonly ModelCatalogItem[],
  query: string,
  folder: string,
): readonly ModelCatalogItem[] {
  const terms = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  return items.filter(
    (item) =>
      (folder === '' || item.folder === folder || item.folder.startsWith(`${folder}/`)) &&
      terms.every((term) =>
        [item.title, item.version, item.value, item.baseModel, ...item.trainedWords]
          .join(' ')
          .toLocaleLowerCase()
          .includes(term),
      ),
  );
}

/** Return immediate child folders without maintaining a second mutable folder index. */
export function modelChoiceFolders(
  items: readonly ModelCatalogItem[],
  folder: string,
): readonly string[] {
  const prefix = folder ? `${folder}/` : '';
  const children = new Set<string>();
  for (const item of items) {
    if (!item.folder.startsWith(prefix)) continue;
    const child = item.folder.slice(prefix.length).split('/')[0];
    if (child) children.add(`${prefix}${child}`);
  }
  return [...children].sort((left, right) => left.localeCompare(right));
}
