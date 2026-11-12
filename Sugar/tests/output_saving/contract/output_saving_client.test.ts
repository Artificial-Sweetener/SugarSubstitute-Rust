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
/** Prove validated saving transport, deliberate migration and nonmutating draft requests. */
import { jest } from '@jest/globals';
import type { CubeLibraryApi } from '../../../frontend/comfyui/ui/core/CubeLibraryApi.js';
import { OutputSavingClient } from '../../../frontend/comfyui/ui/output_saving/OutputSavingClient.js';
import { decodeOutputSavingPreferences } from '../../../frontend/comfyui/ui/output_saving/OutputSavingPreferences.js';
import { savingPreferences, savingResponse } from '../support/SavingSettingsFixture.js';

test('validates complete server snapshots and submits drafts separately from deliberate save and import', async () => {
  const fetchJson = jest.fn<CubeLibraryApi['fetchJson']>().mockResolvedValue(savingResponse());
  const client = new OutputSavingClient({ fetchJson });
  expect((await client.load()).preferences).toEqual(savingPreferences);
  const signal = new AbortController().signal;
  await client.preview(savingPreferences, signal);
  await client.save(savingPreferences);
  await client.importSubstitute({
    schema_version: '4',
    organization: { path_pattern: '{source}' },
  });
  expect(fetchJson.mock.calls.map(([url]) => url)).toEqual(
    ['settings', 'preview', 'settings', 'import'].map(
      (action) => '/sugarcubes/output-saving/' + action,
    ),
  );
  expect(fetchJson.mock.calls[0]?.[1]?.method).toBeUndefined();
  expect(fetchJson.mock.calls[1]?.[1]?.method).toBe('POST');
  expect(fetchJson.mock.calls[1]?.[1]?.body).toBe(JSON.stringify(savingPreferences));
});

test.each([
  undefined,
  {},
  { ...savingPreferences, schema_version: 2 },
  { ...savingPreferences, jpeg: { ...savingPreferences.jpeg, quality: 0 } },
  { ...savingPreferences, jpeg: { ...savingPreferences.jpeg, target_size_kib: 1.5 } },
  { ...savingPreferences, manage_workflow: 'yes' },
])('rejects malformed policy without manufacturing default settings: %j', (value) => {
  expect(() => decodeOutputSavingPreferences(value)).toThrow();
});

test('preserves actionable literal server errors without browser-native dialogs or preference writes', async () => {
  const fetchJson = jest.fn<CubeLibraryApi['fetchJson']>().mockResolvedValue({
    response: { ok: false },
    data: { success: false, error: { message: '<bad path>' } },
  });
  await expect(new OutputSavingClient({ fetchJson }).load()).rejects.toThrow('<bad path>');
  expect(fetchJson).toHaveBeenCalledTimes(1);
});

test('forwards caller cancellation and releases timeout ownership after a completed request', async () => {
  jest.useFakeTimers();
  try {
    const abort = new AbortController();
    let observed: AbortSignal | undefined;
    const fetchJson: CubeLibraryApi['fetchJson'] = async (_path, options) => {
      observed = options?.signal ?? undefined;
      abort.abort();
      return savingResponse();
    };
    await new OutputSavingClient({ fetchJson }).load(abort.signal);
    expect(observed?.aborted).toBe(true);
    expect(jest.getTimerCount()).toBe(0);
  } finally {
    jest.useRealTimers();
  }
});
