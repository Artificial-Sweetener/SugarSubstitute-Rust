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
/** Prove the wildcard folder's catalog, lazy candidates and queue-time resolution. */
import { jest } from '@jest/globals';
import { TextEncoder } from 'node:util';
import { prepareWildcardGeneration } from '../../../frontend/sugarprompt/services/src/index.js';
import { WildcardLibrary } from '../../../frontend/comfyui/ui/wildcards/application/WildcardLibrary.js';
import { resolveWildcardTexts } from '../../../frontend/comfyui/ui/wildcards/application/WildcardPromptResolution.js';
import {
  wildcardFile,
  type WildcardFile,
} from '../../../frontend/comfyui/ui/wildcards/domain/WildcardSource.js';

// The tagged-pick digest encodes text; the browser test environment omits the encoder.
Object.assign(globalThis, { TextEncoder });

/** A wildcard folder held in memory; every list and read is counted. */
function folder(initial: Record<string, string>) {
  const texts = new Map(Object.entries(initial));
  const stamps = new Map<string, number>();
  const list = jest.fn(
    async (): Promise<readonly WildcardFile[]> =>
      [...texts.keys()].flatMap((path) => {
        const file = wildcardFile(path, String(stamps.get(path) ?? 0));
        return file ? [file] : [];
      }),
  );
  const read = jest.fn(async (path: string) => texts.get(path) ?? null);
  return {
    list,
    read,
    write(path: string, text: string) {
      texts.set(path, text);
      stamps.set(path, (stamps.get(path) ?? 0) + 1);
    },
    remove: (path: string) => texts.delete(path),
  };
}

const COLLECTION = {
  'colors.txt': 'red\ngreen\nblue\n',
  'Animals/Pets.txt': 'cat\ndog\nbird\nfish\n',
  'outfit.txt': 'a {colors} coat\n',
  'people.csv': 'Name,Hair\nAlice,red\nBob,black\nCara,blue\n',
  'readme.md': 'not a wildcard',
};

test('the catalog lists TXT names and CSV columns and changes only when a file does', async () => {
  const store = folder(COLLECTION);
  const report = jest.fn();
  const library = new WildcardLibrary(store, report);
  const published = jest.fn();
  library.subscribe(published);
  expect(library.catalog).toBeNull();
  await Promise.all([library.refresh(), library.refresh()]);
  expect(store.list).toHaveBeenCalledTimes(1);
  // Only the CSV file is read, and only for its header.
  expect(store.read.mock.calls).toEqual([['people.csv']]);
  expect(published).toHaveBeenCalledTimes(1);
  const catalog = library.catalog!;
  expect(catalog.status).toBe('ready');
  expect(catalog.resolve({ form: 'simple', identifier: 'Animals/Pets', csvColumn: null })).toEqual(
    expect.objectContaining({ status: 'found' }),
  );
  expect(catalog.resolve({ form: 'csv', identifier: 'people', csvColumn: 'hair' })).toEqual(
    expect.objectContaining({ status: 'found', matchedCsvColumn: 'Hair' }),
  );
  expect(catalog.resolve({ form: 'simple', identifier: 'readme', csvColumn: null })).toEqual(
    expect.objectContaining({ status: 'missing' }),
  );
  const search = catalog.search('co');
  expect(search.status === 'ready' && search.suggestions.map((item) => item.tag)).toContain(
    'colors',
  );

  // An unchanged folder publishes nothing and reads nothing.
  await library.refresh();
  expect(published).toHaveBeenCalledTimes(1);
  expect(store.read).toHaveBeenCalledTimes(1);
  // A new file publishes a new catalog without reading the unchanged CSV again.
  store.write('moods.txt', 'calm\n');
  await library.refresh();
  expect(published).toHaveBeenCalledTimes(2);
  expect(library.catalog).not.toBe(catalog);
  expect(store.read).toHaveBeenCalledTimes(1);
  expect(report).not.toHaveBeenCalled();
});

test('a folder that cannot be listed is reported and offers an unavailable catalog', async () => {
  const failure = new Error('storage offline');
  const report = jest.fn();
  const library = new WildcardLibrary(
    { list: async () => Promise.reject(failure), read: async () => null },
    report,
  );
  await library.refresh();
  expect(report).toHaveBeenCalledWith(failure);
  expect(library.catalog?.status).toBe('unavailable');
  expect(await library.candidates('simple', 'colors')).toBeNull();
});

test('candidates are read when a run names them and kept until their file changes', async () => {
  const store = folder(COLLECTION);
  const library = new WildcardLibrary(store, jest.fn());
  await library.refresh();
  store.read.mockClear();
  const pets = await library.candidates('simple', 'ANIMALS//pets');
  expect(pets).toEqual({
    status: 'ready',
    form: 'simple',
    identifier: 'animals/pets',
    // Tagged picks hash this: the wildcard's name, never where its file lives.
    sourceId: 'animals/pets',
    lines: ['cat', 'dog', 'bird', 'fish'],
  });
  expect(await library.candidates('simple', 'animals/pets')).toBe(pets);
  expect(store.read).toHaveBeenCalledTimes(1);
  expect(await library.candidates('simple', 'people')).toBeNull();
  expect(await library.candidates('csv', 'people')).toMatchObject({
    form: 'csv',
    sourceId: 'people',
    rows: [
      { name: 'Alice', hair: 'red' },
      { name: 'Bob', hair: 'black' },
      { name: 'Cara', hair: 'blue' },
    ],
  });
  store.write('Animals/Pets.txt', 'horse\n');
  await library.refresh();
  expect(await library.candidates('simple', 'animals/pets')).toMatchObject({ lines: ['horse'] });
});

test('a run resolves every text with one seed and reads only the wildcards it names', async () => {
  const store = folder(COLLECTION);
  const library = new WildcardLibrary(store, jest.fn());
  const texts = [
    'a {colors} bird',
    'wearing {outfit}, {colors} eyes, {unknown} stays, {csv:people:hair} hair',
    'no wildcards here',
  ];
  const resolved = await resolveWildcardTexts(texts, '42', '7', library);
  // Python's random.Random(42).randrange(3) is 2, so the first wildcard met picks "blue"; the
  // same wildcard picks the same candidate in every field and inside a nested candidate.
  expect(resolved.get(texts[0]!)).toBe('a blue bird');
  const second = resolved.get(texts[1]!)!;
  expect(second).toMatch(
    /^wearing a blue coat, blue eyes, \{unknown\} stays, (red|black|blue) hair$/u,
  );
  expect(resolved.has(texts[2]!)).toBe(false);
  // The pets wildcard was never named, so its file was never read.
  expect(store.read.mock.calls.map(([path]) => path).sort()).toEqual([
    'colors.txt',
    'outfit.txt',
    'people.csv',
    'people.csv',
  ]);
  // The same texts and seed resolve identically again.
  expect(await resolveWildcardTexts(texts, '42', '9', library)).toEqual(resolved);
});

test('a tagged pick depends on the wildcard name, the seed and the tag alone', async () => {
  const library = new WildcardLibrary(folder(COLLECTION), jest.fn());
  const text = '{animals/pets|left} and {animals/pets|right} and {animals/pets}';
  const resolved = await resolveWildcardTexts([text], '42', '7', library);
  const direct = prepareWildcardGeneration({
    revision: 'reference',
    seed: '42',
    sources: [
      {
        status: 'ready',
        form: 'simple',
        identifier: 'animals/pets',
        sourceId: 'animals/pets',
        lines: ['cat', 'dog', 'bird', 'fish'],
      },
    ],
  });
  if (direct.status !== 'ready') throw new Error('The reference pass was rejected.');
  const reference = direct.pass.resolve(text);
  direct.pass.dispose();
  expect(reference.status).toBe('resolved');
  expect(resolved.get(text)).toBe(reference.status === 'resolved' ? reference.resolvedText : '');
});

test('texts without an opening brace never touch the folder, and a seedless run still resolves', async () => {
  const store = folder(COLLECTION);
  const library = new WildcardLibrary(store, jest.fn());
  expect(
    (await resolveWildcardTexts(['plain text', '(weighted:1.2)'], '1', '2', library)).size,
  ).toBe(0);
  expect(store.list).not.toHaveBeenCalled();
  const seedless = await resolveWildcardTexts(['{colors}'], null, '12345', library);
  expect(['red', 'green', 'blue']).toContain(seedless.get('{colors}'));
});
