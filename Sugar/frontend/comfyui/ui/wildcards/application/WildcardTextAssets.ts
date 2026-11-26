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
/** The wildcard folder as the full-window file editor shows and changes it. */
import { wildcardFile, type WildcardFile } from '../domain/WildcardSource.js';
import type { TextAsset, TextAssetSource } from '../../text_assets/domain/TextAsset.js';

/** The folder's files, as the editor reads and changes them. */
export interface WildcardFileActions {
  list(): Promise<readonly WildcardFile[]>;
  read(path: string): Promise<string | null>;
  create(path: string, text: string): Promise<'created' | 'exists'>;
  write(path: string, text: string): Promise<void>;
  rename(from: string, to: string): Promise<'renamed' | 'exists' | 'missing'>;
  delete(path: string): Promise<void>;
}

const GROUPS = { text: 'TXT Wildcards', csv: 'CSV Wildcards' } as const;
const SUFFIX = { text: '.txt', csv: '.csv' } as const;

/** A path's name without its extension, as the list labels it. */
function label(file: WildcardFile): string {
  return file.path.slice(0, file.path.lastIndexOf('.'));
}

/** Lines as Python's `splitlines` counts them: a final line break ends a line, not a new one. */
function lineCount(text: string): number {
  if (!text) return 0;
  const lines = text.split(/\r\n|\r|\n/u);
  return lines.at(-1) === '' ? lines.length - 1 : lines.length;
}

/** A typed name as a file path of `kind`, any typed `.txt` or `.csv` ending replaced. */
function pathOf(name: string, kind: WildcardFile['kind']): string {
  return (
    name
      .trim()
      .replaceAll('\\', '/')
      .replace(/\.(?:txt|csv)$/iu, '') + SUFFIX[kind]
  );
}

/**
 * Wildcard files for the editor. Wildcards have no enabled state: a prompt names the ones it uses.
 * Applying the editor reads the folder again, so every prompt editor and run sees the new files.
 */
export function wildcardTextAssets(
  files: WildcardFileActions,
  refresh: () => Promise<void>,
): TextAssetSource {
  let listed: readonly WildcardFile[] = [];
  const refusal = (path: string, except: string | null): string | null => {
    const file = wildcardFile(path, '');
    if (!file) return 'Use letters, numbers and folders, such as colors or people/hair.';
    return listed.some(
      (existing) =>
        existing.path !== except &&
        existing.kind === file.kind &&
        existing.identifier === file.identifier,
    )
      ? 'A wildcard with this name already exists.'
      : null;
  };
  const creation = (kind: WildcardFile['kind'], initial: string) => ({
    label: kind === 'text' ? 'New TXT' : 'New CSV',
    refusal: (name: string) => refusal(pathOf(name, kind), null),
    create: async (name: string) => {
      const path = pathOf(name, kind);
      if ((await files.create(path, initial)) === 'exists')
        throw new Error('A wildcard with this name already exists.');
      await refresh();
      return path;
    },
  });
  return {
    title: 'Wildcards',
    listTitle: 'Wildcard files',
    emptyText: 'No user wildcard files yet.',
    creations: [creation('text', ''), creation('csv', 'value\n')],
    async assets() {
      listed = await files.list();
      const sorted = [...listed].sort(
        (a, b) =>
          Number(a.kind === 'csv') - Number(b.kind === 'csv') ||
          label(a).toLowerCase().localeCompare(label(b).toLowerCase()) ||
          a.path.toLowerCase().localeCompare(b.path.toLowerCase()),
      );
      return Promise.all(
        sorted.map(async (file): Promise<TextAsset> => {
          const count = lineCount((await files.read(file.path)) ?? '');
          return {
            id: file.path,
            group: GROUPS[file.kind],
            label: label(file),
            subtitle: `${String(count)} ${count === 1 ? 'wildcard' : 'wildcards'}`,
            enabled: null,
            document: file.kind === 'text' ? 'wildcard-txt' : 'wildcard-csv',
          };
        }),
      );
    },
    read: (id) => files.read(id),
    write: async (id, text) => {
      await files.write(id, text);
      await refresh();
    },
    renameText: (asset) => asset.label,
    renameRefusal: (asset, name) =>
      refusal(pathOf(name, asset.document === 'wildcard-csv' ? 'csv' : 'text'), asset.id),
    async rename(asset, name) {
      const path = pathOf(name, asset.document === 'wildcard-csv' ? 'csv' : 'text');
      const outcome = await files.rename(asset.id, path);
      if (outcome === 'exists') throw new Error('A wildcard with this name already exists.');
      if (outcome === 'missing') throw new Error('That wildcard is no longer in the folder.');
      await refresh();
      return path;
    },
    async delete(asset) {
      await files.delete(asset.id);
      await refresh();
    },
    applied: refresh,
  };
}
