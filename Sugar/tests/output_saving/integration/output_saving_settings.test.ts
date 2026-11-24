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
/** Prove the Output files rows: Python's rows, each change saved at once, dependents live. */
import { beforeEach, jest } from '@jest/globals';
import type { CubeLibraryApi } from '../../../frontend/comfyui/ui/core/CubeLibraryApi.js';
import { OutputSavingClient } from '../../../frontend/comfyui/ui/output_saving/OutputSavingClient.js';
import { OutputSavingSettings } from '../../../frontend/comfyui/ui/output_saving/OutputSavingSettings.js';
import { pickSubstituteSettings } from '../../../frontend/comfyui/ui/output_saving/SubstituteSettingsPicker.js';
import { savingPreferences, savingResponse } from '../support/SavingSettingsFixture.js';
import { drawRows, settle } from '../../settings/support/SettingsRowHarness.js';

const ROWS = [
  'generation.output.folder',
  'generation.output.pattern',
  'generation.output.preview',
  'generation.output.persistence',
  'generation.output.jpeg',
  'generation.output.transfer',
  'generation.output.workflow',
  'generation.output.previews',
  'generation.output.import',
];

beforeEach(() => {
  document.body.replaceChildren();
  localStorage.clear();
});

function mount(fetchJson: CubeLibraryApi['fetchJson']) {
  const report = jest.fn<(error: unknown) => void>();
  const app = {};
  new OutputSavingSettings(document, new OutputSavingClient({ fetchJson }), report, null).register(
    app,
  );
  return { rows: drawRows(app, ROWS, report), report };
}

test('rows show the saved settings and dependent rows follow their switches live', async () => {
  const fetchJson = jest.fn<CubeLibraryApi['fetchJson']>().mockResolvedValue(savingResponse());
  const { rows } = mount(fetchJson);
  try {
    await settle();
    expect(rows.control<HTMLInputElement>('generation.output.pattern', 'input').value).toBe(
      savingPreferences.organization.path_pattern,
    );
    // The example path is shown as text, never parsed as markup.
    expect(rows.control<HTMLInputElement>('generation.output.preview', 'input').value).toContain(
      '<script>',
    );
    expect(rows.root.querySelector('script')).toBeNull();
    const previews = rows.control<HTMLButtonElement>(
      'generation.output.previews',
      '[role="switch"]',
    );
    const transfer = rows.control<HTMLButtonElement>(
      'generation.output.transfer',
      '[role="switch"]',
    );
    expect(previews.disabled).toBe(true);
    expect(transfer.disabled).toBe(true);
    // JPEG companions is a switch expander: its sizing rows show while it is on.
    expect(rows.card('generation.output.jpeg').hasAttribute('data-open')).toBe(false);
    fetchJson.mockResolvedValueOnce(
      savingResponse({ ...savingPreferences, manage_workflow: true }),
    );
    rows.control<HTMLButtonElement>('generation.output.workflow', '[role="switch"]').click();
    await settle();
    expect(previews.disabled).toBe(false);
    fetchJson.mockResolvedValueOnce(
      savingResponse({
        ...savingPreferences,
        manage_workflow: true,
        jpeg: { ...savingPreferences.jpeg, enabled: true },
      }),
    );
    rows.control<HTMLButtonElement>('generation.output.jpeg', '[role="switch"]').click();
    await settle();
    expect(rows.card('generation.output.jpeg').hasAttribute('data-open')).toBe(true);
    expect(transfer.disabled).toBe(false);
  } finally {
    rows.dispose();
  }
});

test('a changed field saves the whole settings at once; a failure is reported and reverts', async () => {
  const fetchJson = jest
    .fn<CubeLibraryApi['fetchJson']>()
    .mockResolvedValueOnce(savingResponse())
    .mockResolvedValueOnce({
      response: { ok: false },
      data: { success: false, error: { message: 'Folder is unavailable' } },
    });
  const { rows, report } = mount(fetchJson);
  try {
    await settle();
    const pattern = rows.control<HTMLInputElement>('generation.output.pattern', 'input');
    pattern.value = '{run}_{source}';
    pattern.dispatchEvent(new Event('change'));
    await settle();
    expect(fetchJson.mock.calls[1]?.[1]?.body).toContain('"path_pattern":"{run}_{source}"');
    expect(report).toHaveBeenLastCalledWith(
      expect.objectContaining({ message: 'Folder is unavailable' }),
    );
    pattern.blur();
    expect(pattern.value).toBe(savingPreferences.organization.path_pattern);
    fetchJson.mockResolvedValueOnce(
      savingResponse({
        ...savingPreferences,
        organization: { ...savingPreferences.organization, path_pattern: '{run}' },
      }),
    );
    pattern.value = '{run}';
    pattern.dispatchEvent(new Event('change'));
    await settle();
    expect(pattern.value).toBe('{run}');
    expect(localStorage.length).toBe(0);
  } finally {
    rows.dispose();
  }
});

test('explicit import picker cancellation releases its owned element and never reads a file', async () => {
  const result = pickSubstituteSettings(document);
  const input = document.querySelector('input[type="file"]');
  if (!(input instanceof HTMLInputElement)) throw new Error('Missing source settings picker.');
  input.dispatchEvent(new Event('cancel'));
  await expect(result).resolves.toBeNull();
  expect(input.isConnected).toBe(false);
});
