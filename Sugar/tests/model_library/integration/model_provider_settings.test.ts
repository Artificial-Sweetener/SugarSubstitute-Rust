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
/** Prove the server-owned provider policy rows: live, never writing defaults, one change each. */
import { jest } from '@jest/globals';
import { ModelProviderSettings } from '../../../frontend/comfyui/ui/model_library/ModelProviderSettings.js';
import {
  ModelProviderPreferenceClient,
  decodeModelProviderPreferences,
} from '../../../frontend/comfyui/ui/model_library/ModelProviderPreferences.js';
import type { ModelApiFetch } from '../../../frontend/comfyui/ui/model_library/ModelApiTransport.js';
import { drawRows, settle } from '../../settings/support/SettingsRowHarness.js';

const initial = {
  schemaVersion: 1,
  metadataLookupEnabled: false,
  missingModelLookupEnabled: true,
  downloadsEnabled: true,
  thumbnailDownloadsEnabled: true,
  thumbnailSafety: 'allow_soft',
  thumbnailRevealOnHover: false,
};
const ROWS = [
  'generation.missing_models.lookup',
  'generation.missing_models.downloads',
  'model_sources.civitai_lookup.local_metadata',
  'model_sources.safety.civitai_thumbnails',
  'model_sources.safety.civitai_thumbnail_policy',
  'model_sources.safety.civitai_thumbnail_reveal',
];

function response(payload: unknown, ok = true): Response {
  return { ok, json: async () => payload } as Response;
}

function mount(fetchApi: ModelApiFetch) {
  const report = jest.fn<(error: unknown) => void>();
  const app = {};
  const settings = new ModelProviderSettings(new ModelProviderPreferenceClient(fetchApi), report);
  settings.setup(app);
  settings.setup(app);
  return { rows: drawRows(app, ROWS, report), report };
}

const switchOf = (rows: ReturnType<typeof mount>['rows'], id: string): HTMLButtonElement =>
  rows.control<HTMLButtonElement>(id, '[role="switch"]');

test('rows show the server policy without writing defaults and send one deliberate change', async () => {
  const calls: (RequestInit | undefined)[] = [];
  let server = initial;
  const { rows, report } = mount(async (_path, options) => {
    calls.push(options);
    if (options?.method === 'PATCH') server = { ...server, metadataLookupEnabled: true };
    return response(server);
  });
  try {
    await settle();
    expect(calls).toHaveLength(1);
    expect(calls[0]?.method).toBeUndefined();
    const metadata = switchOf(rows, 'model_sources.civitai_lookup.local_metadata');
    expect(metadata.getAttribute('aria-checked')).toBe('false');
    expect(metadata.disabled).toBe(false);
    expect(
      rows.control<HTMLSelectElement>('model_sources.safety.civitai_thumbnail_policy', 'select')
        .value,
    ).toBe('allow_soft');
    // A click anywhere on a switch row's card flips it, as in Python.
    rows
      .card('model_sources.civitai_lookup.local_metadata')
      .querySelector<HTMLElement>('.sc-sw-card-title')
      ?.click();
    await settle();
    expect(calls[1]?.method).toBe('PATCH');
    expect(calls[1]?.body).toBe(JSON.stringify({ metadataLookupEnabled: true }));
    expect(metadata.getAttribute('aria-checked')).toBe('true');
    expect(report).not.toHaveBeenCalled();
  } finally {
    rows.dispose();
  }
});

test('thumbnail safety follows the thumbnail switch live', async () => {
  let server = initial;
  const { rows } = mount(async (_path, options) => {
    if (options?.method === 'PATCH')
      server = { ...server, ...(JSON.parse(String(options.body)) as object) };
    return response(server);
  });
  try {
    await settle();
    const safety = rows.control<HTMLSelectElement>(
      'model_sources.safety.civitai_thumbnail_policy',
      'select',
    );
    expect(safety.disabled).toBe(false);
    switchOf(rows, 'model_sources.safety.civitai_thumbnails').click();
    await settle();
    expect(server.thumbnailDownloadsEnabled).toBe(false);
    expect(safety.disabled).toBe(true);
    expect(safety.value).toBe('allow_soft');
  } finally {
    rows.dispose();
  }
});

test('a failed edit is reported and the control returns to the saved policy', async () => {
  let failEdit = true;
  const { rows, report } = mount(async (_path, options) => {
    if (options?.method === 'PATCH') {
      if (failEdit) return response({ error: '<b>Save unavailable</b>' }, false);
      return response({ ...initial, thumbnailSafety: 'sfw_only' });
    }
    return response(initial);
  });
  try {
    await settle();
    const safety = rows.control<HTMLSelectElement>(
      'model_sources.safety.civitai_thumbnail_policy',
      'select',
    );
    safety.value = 'allow_all';
    safety.dispatchEvent(new Event('change', { bubbles: true }));
    await settle();
    expect(safety.value).toBe('allow_soft');
    expect(report).toHaveBeenLastCalledWith(
      expect.objectContaining({ message: '<b>Save unavailable</b>' }),
    );
    expect(rows.root.querySelector('b')).toBeNull();
    failEdit = false;
    safety.value = 'sfw_only';
    safety.dispatchEvent(new Event('change', { bubbles: true }));
    await settle();
    expect(safety.value).toBe('sfw_only');
  } finally {
    rows.dispose();
  }
});

test.each([
  { ...initial, schemaVersion: 2 },
  { ...initial, metadataLookupEnabled: 'false' },
  { ...initial, missingModelLookupEnabled: undefined },
  { ...initial, downloadsEnabled: 1 },
  { ...initial, thumbnailSafety: '<img src=x>' },
  {},
])('rejects malformed shared policy %j', (payload) => {
  expect(() => decodeModelProviderPreferences(payload)).toThrow();
});

test.each([
  ['missingModelLookupEnabled', 'generation.missing_models.lookup'],
  ['downloadsEnabled', 'generation.missing_models.downloads'],
] as const)('persists %s independently of metadata and thumbnails', async (key, row) => {
  let server = initial;
  const patches: unknown[] = [];
  const { rows, report } = mount(async (_path, options) => {
    if (options?.method === 'PATCH') {
      const payload: unknown = JSON.parse(String(options.body));
      patches.push(payload);
      server = { ...server, [key]: false };
    }
    return response(server);
  });
  try {
    await settle();
    const control = switchOf(rows, row);
    expect(control.getAttribute('aria-checked')).toBe('true');
    control.click();
    await settle();
    expect(patches).toEqual([{ [key]: false }]);
    expect(control.getAttribute('aria-checked')).toBe('false');
    expect(server.metadataLookupEnabled).toBe(false);
    expect(server.thumbnailDownloadsEnabled).toBe(true);
    expect(report).not.toHaveBeenCalled();
  } finally {
    rows.dispose();
  }
});

test('reveal on hover can be switched only while thumbnails are shown blurred', async () => {
  const patches: unknown[] = [];
  let server = initial;
  const { rows } = mount(async (_path, options) => {
    if (options?.method === 'PATCH') {
      const patch: unknown = JSON.parse(String(options.body));
      patches.push(patch);
      server = { ...server, ...(patch as Partial<typeof initial>) };
    }
    return response(server);
  });
  try {
    await settle();
    const reveal = (): HTMLButtonElement =>
      switchOf(rows, 'model_sources.safety.civitai_thumbnail_reveal');
    // Nothing is blurred under "Allow soft / racy", so there is nothing to reveal.
    expect(reveal().disabled).toBe(true);
    const policy = rows.control<HTMLSelectElement>(
      'model_sources.safety.civitai_thumbnail_policy',
      'select',
    );
    expect([...policy.options].map((option) => option.value)).toEqual([
      'sfw_only',
      'allow_soft',
      'blur',
      'allow_all',
      'disabled',
    ]);
    policy.value = 'blur';
    policy.dispatchEvent(new Event('change', { bubbles: true }));
    await settle();
    expect(reveal().disabled).toBe(false);
    expect(reveal().getAttribute('aria-checked')).toBe('false');
    reveal().click();
    await settle();
    expect(patches).toEqual([{ thumbnailSafety: 'blur' }, { thumbnailRevealOnHover: true }]);
    expect(reveal().getAttribute('aria-checked')).toBe('true');
  } finally {
    rows.dispose();
  }
});
