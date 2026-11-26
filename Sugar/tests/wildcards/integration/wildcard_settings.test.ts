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
/** Prove the Manage wildcards row and the wildcard files the full-window editor works on. */
import { jest } from '@jest/globals';
import {
  ComfyWildcardFiles,
  type WildcardApiResponse,
} from '../../../frontend/comfyui/ui/wildcards/infrastructure/ComfyWildcardFiles.js';
import { registerWildcardSettings } from '../../../frontend/comfyui/ui/wildcards/presentation/WildcardSettings.js';
import { wildcardTextAssets } from '../../../frontend/comfyui/ui/wildcards/application/WildcardTextAssets.js';
import { drawRows } from '../../settings/support/SettingsRowHarness.js';
import { wildcardFile } from '../../../frontend/comfyui/ui/wildcards/domain/WildcardSource.js';

afterEach(() => document.body.replaceChildren());

/** An in-memory wildcard folder with the editor's file actions over it. */
function folder(files: Record<string, string>) {
  const contents = new Map(Object.entries(files));
  const refresh = jest.fn(async () => undefined);
  const source = wildcardTextAssets(
    {
      list: async () =>
        [...contents.keys()].flatMap((path) => {
          const file = wildcardFile(path, String(contents.get(path)?.length));
          return file ? [file] : [];
        }),
      read: async (path) => contents.get(path) ?? null,
      create: async (path, text) => {
        if (contents.has(path)) return 'exists';
        contents.set(path, text);
        return 'created';
      },
      write: async (path, text) => void contents.set(path, text),
      rename: async (from, to) => {
        if (!contents.has(from)) return 'missing';
        if (contents.has(to)) return 'exists';
        contents.set(to, contents.get(from) ?? '');
        contents.delete(from);
        return 'renamed';
      },
      delete: async (path) => void contents.delete(path),
    },
    refresh,
  );
  return { contents, refresh, source };
}

test('the Manage wildcards row offers Manage, Open folder and Refresh', async () => {
  const app = {};
  const manage = jest.fn(async () => undefined);
  const openFolder = jest.fn(async () => undefined);
  const refresh = jest.fn(async () => undefined);
  registerWildcardSettings({ app, manage, openFolder, refresh });
  const report = jest.fn();
  const row = drawRows(app, ['prompt_editing.wildcards.manage'], report).card(
    'prompt_editing.wildcards.manage',
  );
  const buttons = [...row.querySelectorAll('button')];
  expect(buttons.map((button) => button.textContent)).toEqual(['Manage', 'Open folder', 'Refresh']);
  for (const button of buttons) button.click();
  for (let turn = 0; turn < 5; turn++) await Promise.resolve();
  expect([manage, openFolder, refresh].map((action) => action.mock.calls.length)).toEqual([
    1, 1, 1,
  ]);
  expect(report).not.toHaveBeenCalled();
});

test('files are listed TXT first, then CSV, each with its line count', async () => {
  const f = folder({
    'sets/moods.csv': 'value\ncalm\n',
    'colors.txt': 'red\ngreen\nblue\n',
    'Animals.txt': 'cat',
  });
  expect(await f.source.assets()).toEqual([
    {
      id: 'Animals.txt',
      group: 'TXT Wildcards',
      label: 'Animals',
      subtitle: '1 wildcard',
      enabled: null,
      document: 'wildcard-txt',
    },
    expect.objectContaining({ id: 'colors.txt', subtitle: '3 wildcards' }),
    expect.objectContaining({
      id: 'sets/moods.csv',
      group: 'CSV Wildcards',
      label: 'sets/moods',
      subtitle: '2 wildcards',
      document: 'wildcard-csv',
    }),
  ]);
});

test('New TXT and New CSV make files of their kind, refusing taken and unsafe names', async () => {
  const f = folder({ 'colors.txt': '' });
  await f.source.assets();
  const [txt, csv] = f.source.creations;
  expect([txt?.label, csv?.label]).toEqual(['New TXT', 'New CSV']);
  expect(txt?.refusal('Colors')).toBe('A wildcard with this name already exists.');
  expect(txt?.refusal('../escape')).toBe(
    'Use letters, numbers and folders, such as colors or people/hair.',
  );
  // A CSV may share a TXT's name: a prompt names them apart.
  expect(csv?.refusal('colors')).toBeNull();
  expect(await txt?.create('people/hair.csv')).toBe('people/hair.txt');
  expect(await csv?.create('sets\\moods')).toBe('sets/moods.csv');
  expect(f.contents.get('people/hair.txt')).toBe('');
  expect(f.contents.get('sets/moods.csv')).toBe('value\n');
  expect(f.refresh).toHaveBeenCalledTimes(2);
});

test('a renamed file keeps its kind, and saving or deleting reads the folder again', async () => {
  const f = folder({ 'People/Hair.csv': 'value\nlong\n', 'colors.txt': 'red' });
  const [, hair] = await f.source.assets();
  if (!hair) throw new Error('missing asset');
  expect(f.source.renameText(hair)).toBe('People/Hair');
  expect(f.source.renameRefusal(hair, 'colors')).toBeNull();
  expect(await f.source.rename(hair, 'people/styles.txt')).toBe('people/styles.csv');
  await f.source.write('colors.txt', 'red\nblue');
  const [colors] = await f.source.assets();
  if (!colors) throw new Error('missing asset');
  await f.source.delete(colors);
  expect([...f.contents.keys()]).toEqual(['people/styles.csv']);
  expect(f.refresh).toHaveBeenCalledTimes(3);
});

test('opening the folder asks the backend, and its refusal reaches the person', async () => {
  const answer = (status: number, body: unknown): WildcardApiResponse => ({
    ok: status === 200,
    status,
    json: async () => body,
    text: async () => '',
  });
  const fetchApi = jest
    .fn<(path: string, options?: RequestInit) => Promise<WildcardApiResponse>>()
    .mockResolvedValueOnce(answer(200, { opened: true }))
    .mockResolvedValueOnce(
      answer(501, {
        error: { message: 'This computer has no file manager to open the folder in.' },
      }),
    );
  const files = new ComfyWildcardFiles(fetchApi);
  await files.openFolder();
  expect(fetchApi.mock.calls[0]?.[0]).toBe('/sugarcubes/wildcards/open_folder');
  expect(fetchApi.mock.calls[0]?.[1]).toMatchObject({ method: 'POST' });
  await expect(files.openFolder()).rejects.toThrow(
    'This computer has no file manager to open the folder in.',
  );
});

test('saving and deleting a file go through Comfy’s per-user file routes', async () => {
  const fetchApi = jest.fn(
    async (): Promise<WildcardApiResponse> => ({
      ok: true,
      status: 200,
      json: async () => null,
      text: async () => '',
    }),
  );
  const files = new ComfyWildcardFiles(fetchApi);
  await files.write('people/hair.txt', 'long');
  await files.delete('people/hair.txt');
  const calls = fetchApi.mock.calls as unknown as [string, RequestInit][];
  expect(calls.map(([path, init]) => [path, init.method])).toEqual([
    ['/userdata/sugar%2Fwildcards%2Fpeople%2Fhair.txt?overwrite=true', 'POST'],
    ['/userdata/sugar%2Fwildcards%2Fpeople%2Fhair.txt', 'DELETE'],
  ]);
});
