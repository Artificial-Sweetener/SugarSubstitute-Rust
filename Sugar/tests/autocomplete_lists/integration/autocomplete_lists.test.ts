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
/** Prove custom and censored autocomplete lists are kept, turned on and off, and give their tags. */
import { jest } from '@jest/globals';
import {
  AutocompleteListLibrary,
  type AutocompleteListFiles,
} from '../../../frontend/comfyui/ui/autocomplete_lists/application/AutocompleteListLibrary.js';
import { autocompleteListTextAssets } from '../../../frontend/comfyui/ui/autocomplete_lists/application/AutocompleteListTextAssets.js';

/** An in-memory lists folder; each write changes a file's stamp. */
function folder(files: Record<string, string>) {
  const contents = new Map(Object.entries(files));
  let writes = 0;
  const stamps = new Map<string, number>();
  const files_: AutocompleteListFiles = {
    list: async () =>
      [...contents.keys()].map((path) => ({ path, stamp: String(stamps.get(path) ?? 0) })),
    read: async (path) => contents.get(path) ?? null,
    create: async (path, text) => {
      if (contents.has(path)) return 'exists';
      contents.set(path, text);
      return 'created';
    },
    write: async (path, text) => {
      contents.set(path, text);
      stamps.set(path, ++writes);
    },
    rename: async (from, to) => {
      if (contents.has(to)) return 'exists';
      contents.set(to, contents.get(from) ?? '');
      contents.delete(from);
      return 'renamed';
    },
    delete: async (path) => void contents.delete(path),
  };
  const library = new AutocompleteListLibrary(files_, jest.fn());
  return { contents, library, source: autocompleteListTextAssets(library) };
}

test('enabled lists give their tags in list order; disabled lists give none', async () => {
  const f = folder({
    'custom/b.txt': 'second tag\n\n',
    'custom/a.txt': '  first_tag \nanother\n',
    'censored/hide.txt': 'bad tag\n',
    'lists.json': JSON.stringify({ version: 1, disabled: ['custom/b.txt'] }),
    'custom/notes.md': 'not a list',
  });
  const heard = jest.fn();
  f.library.subscribe(heard);
  await f.library.refresh();
  expect(f.library.current()).toMatchObject({
    custom: ['first_tag', 'another'],
    censored: ['bad tag'],
  });
  expect(heard).toHaveBeenCalledTimes(1);
  expect(await f.source.assets()).toEqual([
    {
      id: 'custom/a.txt',
      group: 'Custom tag lists',
      label: 'a',
      subtitle: '2 tags · Enabled',
      enabled: true,
      document: 'tag-list',
    },
    expect.objectContaining({ id: 'custom/b.txt', subtitle: '1 tag · Disabled', enabled: false }),
    expect.objectContaining({ id: 'censored/hide.txt', group: 'Censored tag lists' }),
  ]);
  // Reading an unchanged folder again publishes nothing new.
  expect(heard).toHaveBeenCalledTimes(1);
});

test('turning a list on records it as Python did and republishes its tags', async () => {
  const f = folder({
    'custom/b.txt': 'x',
    'lists.json': '{"version":1,"disabled":["custom/b.txt"]}',
  });
  const [list] = await f.source.assets();
  if (!list) throw new Error('missing list');
  await f.source.setEnabled?.(list, true);
  expect(f.contents.get('lists.json')).toBe('{\n  "version": 1,\n  "disabled": []\n}\n');
  expect(f.library.current().custom).toEqual(['x']);
});

test('new lists are made empty and enabled; taken and unsafe names are refused', async () => {
  const f = folder({ 'custom/mine.txt': 'a' });
  await f.source.assets();
  const [custom, censored] = f.source.creations;
  expect([custom?.label, censored?.label]).toEqual(['New custom list', 'New censored list']);
  expect(custom?.refusal('mine.txt')).toBe('Autocomplete list already exists: mine');
  expect(custom?.refusal('../up')).toBe('Autocomplete list name contains an unsafe path part.');
  expect(censored?.refusal('mine')).toBeNull();
  expect(await censored?.create('people\\hidden.TXT')).toBe('censored/people/hidden.txt');
  expect(f.contents.get('censored/people/hidden.txt')).toBe('');
});

test('a renamed or deleted list keeps or forgets whether it was disabled', async () => {
  const f = folder({
    'custom/old.txt': 'a',
    'custom/other.txt': 'b',
    'lists.json': '{"version":1,"disabled":["custom/old.txt","custom/other.txt"]}',
  });
  const [old, other] = await f.source.assets();
  if (!old || !other) throw new Error('missing list');
  expect(await f.source.rename(old, 'new')).toBe('custom/new.txt');
  await f.source.delete(other);
  expect(JSON.parse(f.contents.get('lists.json') ?? '')).toEqual({
    version: 1,
    disabled: ['custom/new.txt'],
  });
  expect(f.library.lists.map((list) => [list.id, list.enabled])).toEqual([
    ['custom/new.txt', false],
  ]);
});

test('a damaged record disables no list', async () => {
  const f = folder({ 'custom/a.txt': 'tag', 'lists.json': '{not json' });
  await f.library.refresh();
  expect(f.library.current().custom).toEqual(['tag']);
});
