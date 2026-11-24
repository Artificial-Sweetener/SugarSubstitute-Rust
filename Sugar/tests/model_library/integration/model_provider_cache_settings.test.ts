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
/** Prove cache settings use validated server jobs and literal non-blocking feedback. */
import { expect, jest, test } from '@jest/globals';
import { ModelProviderCacheSettings } from '../../../frontend/comfyui/ui/model_library/ModelProviderCacheSettings.js';
import {
  ModelProviderCacheClient,
  decodeModelProviderCacheSummary,
} from '../../../frontend/comfyui/ui/model_library/ModelProviderCacheClient.js';
import type { ModelApiFetch } from '../../../frontend/comfyui/ui/model_library/ModelApiTransport.js';
import { drawRows, settle } from '../../settings/support/SettingsRowHarness.js';

const initial = {
  schemaVersion: 1,
  providerRecordCount: 3,
  thumbnailSourceCount: 2,
  thumbnailVariantCount: 6,
  thumbnailBytes: 4096,
};

/** Supply only response methods used by the external transport boundary. */
function response(value: unknown, ok = true): Response {
  return { ok, json: async () => value } as Response;
}

/** Draw the usage and maintenance rows the cache settings contribute. */
function mount(fetchApi: ModelApiFetch) {
  const report = jest.fn<(error: unknown) => void>();
  const app = {};
  const settings = new ModelProviderCacheSettings(new ModelProviderCacheClient(fetchApi), report);
  settings.setup(app);
  settings.setup(app);
  const rows = drawRows(
    app,
    [
      'model_sources.provider_cache.civitai_usage',
      'model_sources.provider_cache.civitai_maintenance',
    ],
    report,
  );
  return { root: rows.root, rows, report };
}
test.each(['clear_thumbnails', 'clear_metadata', 'refresh'] as const)(
  'runs %s only after its deliberate click and shows committed usage',
  async (action) => {
    const calls: { path: string; method: string | undefined }[] = [];
    let finish: ((value: Response) => void) | null = null;
    const harness = mount(async (path, options) => {
      calls.push({ path, method: options?.method });
      if (options?.method === 'POST')
        return await new Promise<Response>((resolve) => {
          finish = resolve;
        });
      return response(initial);
    });
    try {
      await settle();
      expect(calls).toEqual([{ path: '/sugarcubes/v1/models/cache/civitai', method: undefined }]);
      expect(harness.root.textContent).toContain(
        '3 metadata records · 2 thumbnails · 6 variants · 4.0 KB',
      );
      const buttons = Array.from(harness.root.querySelectorAll('button'));
      expect(buttons.map((button) => button.textContent)).toEqual([
        'Clear thumbnails',
        'Clear metadata',
        'Refresh',
      ]);
      const button = buttons[['clear_thumbnails', 'clear_metadata', 'refresh'].indexOf(action)];
      if (!button) throw new Error('Missing action.');
      button.click();
      button.click();
      expect(buttons.every((item) => item.disabled)).toBe(true);
      expect(calls).toHaveLength(2);
      expect(calls[1]).toEqual({
        path: `/sugarcubes/v1/models/cache/civitai/${action}`,
        method: 'POST',
      });
      if (!finish) throw new Error('Missing action response.');
      const complete: (value: Response) => void = finish;
      complete(
        response({
          jobId: 'a'.repeat(32),
          status: 'completed',
          result: {
            ...initial,
            thumbnailSourceCount: 0,
            thumbnailVariantCount: 0,
            thumbnailBytes: 0,
          },
        }),
      );
      await settle();
      expect(harness.root.textContent).toContain(
        '3 metadata records · 0 thumbnails · 0 variants · 0 B',
      );
      expect(buttons.every((item) => !item.disabled)).toBe(true);
      expect(harness.report).not.toHaveBeenCalled();
    } finally {
      harness.rows.dispose();
    }
  },
);

test('retries initial usage failure and reports failed jobs without rendering error markup or blocking dialogs', async () => {
  let reads = 0;
  const harness = mount(async (_path, options) => {
    if (options?.method === 'POST')
      return response({
        jobId: 'b'.repeat(32),
        status: 'failed',
        error: '<img src=x> failed',
        result: {},
      });
    if (++reads === 1) throw new Error('Offline');
    return response(initial);
  });
  const alert = jest.spyOn(window, 'alert');
  const confirm = jest.spyOn(window, 'confirm');
  try {
    await settle();
    // An unavailable read leaves the actions free, so Refresh can retry it.
    expect(harness.root.textContent).toContain('Cache usage unavailable.');
    expect(
      Array.from(harness.root.querySelectorAll('button')).every((item) => !item.disabled),
    ).toBe(true);
    harness.root.querySelector('button')?.click();
    await settle();
    expect(harness.report).toHaveBeenLastCalledWith(
      expect.objectContaining({ message: '<img src=x> failed' }),
    );
    expect(harness.root.querySelector('img')).toBeNull();
    expect(alert).not.toHaveBeenCalled();
    expect(confirm).not.toHaveBeenCalled();
    expect(
      Array.from(harness.root.querySelectorAll('button')).every((item) => !item.disabled),
    ).toBe(true);
  } finally {
    alert.mockRestore();
    confirm.mockRestore();
    harness.root.remove();
  }
});

test.each([
  {},
  { ...initial, schemaVersion: 2 },
  { ...initial, thumbnailBytes: -1 },
  { ...initial, providerRecordCount: '0' },
  { ...initial, thumbnailVariantCount: 1.5 },
  { ...initial, thumbnailSourceCount: Number.MAX_SAFE_INTEGER + 1 },
])('rejects malformed cache evidence %j', (value) => {
  expect(() => decodeModelProviderCacheSummary(value)).toThrow();
});
