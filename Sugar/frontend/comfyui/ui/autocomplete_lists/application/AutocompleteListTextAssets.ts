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
/** The autocomplete lists as the full-window file editor shows and changes them. */
import type { TextAsset, TextAssetSource } from '../../text_assets/domain/TextAsset.js';
import {
  autocompleteListId,
  autocompleteListName,
  listNameRefusal,
  type AutocompleteListKind,
} from '../domain/AutocompleteList.js';
import type { AutocompleteListLibrary } from './AutocompleteListLibrary.js';

const GROUPS = { custom: 'Custom tag lists', censored: 'Censored tag lists' } as const;

/**
 * Custom and censored tag lists for the editor. Every change goes through the library, which
 * republishes the tags, so suggestions change as soon as a list is saved or turned on or off.
 */
export function autocompleteListTextAssets(library: AutocompleteListLibrary): TextAssetSource {
  const refusal = (kind: AutocompleteListKind, typed: string, except: string | null) => {
    const name = autocompleteListName(typed);
    const unsafe = listNameRefusal(name);
    if (unsafe) return unsafe;
    const id = autocompleteListId(kind, name);
    return library.lists.some((list) => list.id === id && list.id !== except)
      ? `Autocomplete list already exists: ${name}`
      : null;
  };
  const kindOf = (asset: TextAsset): AutocompleteListKind =>
    asset.id.startsWith('censored/') ? 'censored' : 'custom';
  const creation = (kind: AutocompleteListKind) => ({
    label: kind === 'custom' ? 'New custom list' : 'New censored list',
    refusal: (name: string) => refusal(kind, name, null),
    create: (name: string) => library.create(kind, autocompleteListName(name)),
  });
  return {
    title: 'Autocomplete Lists',
    listTitle: 'Tag lists',
    emptyText: 'No custom or censored tag lists yet.',
    creations: [creation('custom'), creation('censored')],
    async assets() {
      await library.refresh();
      return library.lists.map((list): TextAsset => {
        const count = library.tagCount(list.id);
        return {
          id: list.id,
          group: GROUPS[list.kind],
          label: list.name,
          subtitle: `${String(count)} ${count === 1 ? 'tag' : 'tags'} · ${list.enabled ? 'Enabled' : 'Disabled'}`,
          enabled: list.enabled,
          document: 'tag-list',
        };
      });
    },
    read: (id) => library.read(id),
    write: (id, text) => library.write(id, text),
    renameText: (asset) => asset.label,
    renameRefusal: (asset, name) => refusal(kindOf(asset), name, asset.id),
    rename: (asset, name) => library.rename(asset.id, autocompleteListName(name)),
    delete: (asset) => library.delete(asset.id),
    setEnabled: (asset, enabled) => library.setEnabled(asset.id, enabled),
    applied: () => library.refresh(),
  };
}
