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
/** Prove the settings row that opens the wildcard folder and creates and renames wildcards. */
import { jest } from '@jest/globals';
import {
  ComfyWildcardFiles,
  type WildcardApiResponse,
} from '../../../frontend/comfyui/ui/wildcards/infrastructure/ComfyWildcardFiles.js';
import {
  WILDCARD_SETTING,
  registerWildcardSettings,
  type WildcardNamePrompt,
} from '../../../frontend/comfyui/ui/wildcards/presentation/WildcardSettings.js';
import {
  wildcardFile,
  type WildcardFile,
} from '../../../frontend/comfyui/ui/wildcards/domain/WildcardSource.js';

afterEach(() => document.body.replaceChildren());

/** Mount the row over an in-memory folder and answer each name prompt from a script. */
function fixture(paths: string[], answers: (string | null)[]) {
  const folder = new Set(paths);
  const listed = (): WildcardFile[] =>
    [...folder].flatMap((path) => {
      const file = wildcardFile(path, '');
      return file ? [file] : [];
    });
  let files = listed();
  const prompts: Parameters<WildcardNamePrompt>[0][] = [];
  const refusals: (string | null)[] = [];
  const settings: { id: string; type: () => HTMLElement; category: string[] }[] = [];
  const report = jest.fn<(error: unknown) => void>();
  const openFolder = jest.fn(async () => undefined);
  registerWildcardSettings({
    app: {
      ui: {
        settings: { addSetting: (setting: (typeof settings)[number]) => settings.push(setting) },
      },
    },
    document,
    files: () => files,
    refresh: async () => {
      files = listed();
    },
    admit: (path) => wildcardFile(path, ''),
    create: async (path) => {
      if (folder.has(path)) return 'exists';
      folder.add(path);
      return 'created';
    },
    rename: async (from, to) => {
      if (!folder.has(from)) return 'missing';
      if (folder.has(to)) return 'exists';
      folder.delete(from);
      folder.add(to);
      return 'renamed';
    },
    openFolder,
    prompt: async (prompt) => {
      prompts.push(prompt);
      const answer = answers.shift() ?? null;
      if (answer !== null) refusals.push(prompt.validate(answer));
      return answer !== null && prompt.validate(answer) === null ? answer : null;
    },
    report,
  });
  const setting = settings[0]!;
  const row = setting.type();
  document.body.append(row);
  const press = async (label: string): Promise<void> => {
    const button = [...row.querySelectorAll('button')].find((item) => item.textContent === label);
    button!.click();
    for (let turn = 0; turn < 20; turn++) await Promise.resolve();
  };
  return { setting, row, folder, prompts, refusals, report, openFolder, press };
}

test('the row sits under Prompt editor and offers the three wildcard actions', async () => {
  const f = fixture([], []);
  expect(f.setting.id).toBe(WILDCARD_SETTING);
  expect(f.setting.category).toEqual(['SugarCubes', 'Wildcards', 'Wildcards']);
  expect(f.setting.type()).toBe(f.row);
  expect([...f.row.querySelectorAll('button')].map((button) => button.textContent)).toEqual([
    'Open wildcards folder',
    'New wildcard…',
    'Rename wildcard…',
  ]);
  await f.press('Open wildcards folder');
  expect(f.openFolder).toHaveBeenCalledTimes(1);
  expect(f.report).not.toHaveBeenCalled();
});

test('a new wildcard is a TXT file unless its name ends in .csv, and a taken name is refused', async () => {
  const f = fixture(['colors.txt'], ['people/hair', 'Sets/Moods.csv', 'Colors', '../escape']);
  await f.press('New wildcard…');
  await f.press('New wildcard…');
  await f.press('New wildcard…');
  await f.press('New wildcard…');
  expect([...f.folder].sort()).toEqual(['Sets/Moods.csv', 'colors.txt', 'people/hair.txt']);
  expect(f.refusals).toEqual([
    null,
    null,
    'A wildcard with this name already exists.',
    'Use letters, numbers and folders, such as colors or people/hair.',
  ]);
  expect(f.prompts[0]).toMatchObject({ title: 'New wildcard', confirmLabel: 'Create' });
  expect(f.report).not.toHaveBeenCalled();
});

test('a wildcard is renamed by the name prompts use, keeping its kind', async () => {
  const f = fixture(['People/Hair.csv', 'colors.txt'], ['people/hair', 'people/styles', 'nothing']);
  await f.press('Rename wildcard…');
  expect([...f.folder].sort()).toEqual(['colors.txt', 'people/styles.csv']);
  expect(f.prompts[1]).toMatchObject({
    label: 'New name for people/hair',
    initialValue: 'People/Hair',
    confirmLabel: 'Rename',
  });
  // A name the folder does not hold is refused before a new name is asked for.
  await f.press('Rename wildcard…');
  expect(f.refusals.at(-1)).toBe('There is no wildcard with this name.');
  expect(f.prompts).toHaveLength(3);
  expect(f.report).not.toHaveBeenCalled();
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
