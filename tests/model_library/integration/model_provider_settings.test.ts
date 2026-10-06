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
/** Prove server-owned provider settings, right-side Fluent captions and recovery without default writes. */
import { jest } from '@jest/globals';
import { ModelProviderSettings } from '../../../frontend/comfyui/ui/model_library/ModelProviderSettings.js';
import {
  ModelProviderPreferenceClient,
  decodeModelProviderPreferences,
} from '../../../frontend/comfyui/ui/model_library/ModelProviderPreferences.js';
import type { ModelApiFetch } from '../../../frontend/comfyui/ui/model_library/ModelApiTransport.js';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';

/** Isolate FAST's form-associated rendering API, which JSDOM does not implement. */
class FixtureCheckbox extends HTMLElement {
  checked = false;
  disabled = false;

  /** Match Microsoft's indicator-only shadow template, which has no caption slot. */
  constructor() {
    super();
    this.attachShadow({ mode: 'open' });
  }
}
customElements.define('fixture-provider-checkbox', FixtureCheckbox);

/** Supply only the external checkbox control; settings events and state remain real. */
function createCheckbox(document: Document): FixtureCheckbox {
  const checkbox = document.createElement('fixture-provider-checkbox');
  if (!(checkbox instanceof FixtureCheckbox)) throw new Error('Invalid fixture control.');
  return checkbox;
}

const initial = {
  schemaVersion: 1,
  metadataLookupEnabled: false,
  missingModelLookupEnabled: true,
  downloadsEnabled: true,
  thumbnailDownloadsEnabled: true,
  thumbnailSafety: 'allow_soft',
};

/** Model only the fetch response methods consumed at this external test boundary. */
function response(payload: unknown, ok = true): Response {
  return { ok, json: async () => payload } as Response;
}

/** Drain settings promises and Fluent component reactions deterministically. */
async function settle(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
}

/** Capture real host settings renderers without exposing private presentation state. */
function mount(fetchApi: ModelApiFetch) {
  const renderers: (() => unknown)[] = [];
  const report = jest.fn<(error: unknown) => void>();
  const settings = new ModelProviderSettings(
    document,
    new ModelProviderPreferenceClient(fetchApi),
    report,
    createCheckbox,
  );
  const app = {
    ui: {
      settings: {
        addSetting(value: unknown): void {
          if (!isRecord(value) || typeof value.type !== 'function')
            throw new Error('Invalid settings registration.');
          const render = value.type;
          renderers.push(() => render());
        },
      },
    },
  };
  settings.setup(app);
  settings.setup(app);
  const root = document.createElement('div');
  for (const render of renderers) {
    const row = render();
    if (!(row instanceof HTMLElement)) throw new Error('Invalid settings row.');
    root.append(row);
  }
  document.body.append(root);
  return { root, renderers, report };
}

test('reads reference choices without writing defaults and persists only the deliberate checkbox edit', async () => {
  const calls: (RequestInit | undefined)[] = [];
  let server = initial;
  const harness = mount(async (_path, options) => {
    calls.push(options);
    if (options?.method === 'PATCH') server = { ...server, metadataLookupEnabled: true };
    return response(server);
  });
  try {
    await settle();
    expect(harness.renderers).toHaveLength(5);
    expect(calls).toHaveLength(1);
    expect(calls[0]?.method).toBeUndefined();
    const metadata = harness.root.querySelector('fixture-provider-checkbox');
    const safety = harness.root.querySelector('select');
    if (!(metadata instanceof FixtureCheckbox) || !safety) throw new Error('Missing controls.');
    expect(metadata.textContent).toBe('');
    const caption = metadata.nextElementSibling;
    expect(caption?.textContent).toBe('Enabled');
    expect(metadata.parentElement?.style.display).toBe('inline-flex');
    expect(metadata.parentElement?.style.gap).toBe('8px');
    const nativeClick = jest.spyOn(metadata, 'click');
    if (!(caption instanceof HTMLElement)) throw new Error('Missing visible caption.');
    caption.click();
    expect(nativeClick).toHaveBeenCalledTimes(1);
    expect(metadata.closest('.sugarcubes-settings-row')?.getAttribute('style')).toContain(
      '--colorCompoundBrandBackground:',
    );
    expect(metadata.checked).toBe(false);
    expect(metadata.disabled).toBe(false);
    expect(safety.value).toBe('allow_soft');
    metadata.checked = true;
    metadata.dispatchEvent(new Event('change', { bubbles: true }));
    expect(metadata.disabled).toBe(true);
    nativeClick.mockClear();
    caption.click();
    expect(nativeClick).not.toHaveBeenCalled();
    await settle();
    expect(calls[1]?.method).toBe('PATCH');
    expect(calls[1]?.body).toBe(JSON.stringify({ metadataLookupEnabled: true }));
    expect(metadata.checked).toBe(true);
    expect(safety.value).toBe('allow_soft');
    expect(metadata.disabled).toBe(false);
    server = { ...server, thumbnailDownloadsEnabled: false };
    harness.renderers[0]?.();
    await settle();
    expect(safety.disabled).toBe(true);
    expect(safety.value).toBe('allow_soft');
    expect(harness.report).not.toHaveBeenCalled();
  } finally {
    harness.root.remove();
  }
});

test('retains the saved policy on failed edits and retries an unavailable initial read', async () => {
  let reads = 0;
  let failEdit = false;
  const harness = mount(async (_path, options) => {
    if (options?.method === 'PATCH') {
      if (failEdit) return response({ error: '<b>Save unavailable</b>' }, false);
      return response({ ...initial, thumbnailSafety: 'sfw_only' });
    }
    reads++;
    if (reads === 1) throw new Error('Transport unavailable');
    return response(initial);
  });
  try {
    await settle();
    const safety = harness.root.querySelector('select');
    const metadata = harness.root.querySelector('fixture-provider-checkbox');
    if (!safety || !(metadata instanceof FixtureCheckbox)) throw new Error('Missing controls.');
    expect(metadata.disabled).toBe(true);
    harness.renderers[0]?.();
    await settle();
    expect(metadata.disabled).toBe(false);
    failEdit = true;
    safety.value = 'allow_all';
    safety.dispatchEvent(new Event('change', { bubbles: true }));
    await settle();
    expect(safety.value).toBe('allow_soft');
    expect(harness.report).toHaveBeenLastCalledWith(
      expect.objectContaining({ message: '<b>Save unavailable</b>' }),
    );
    expect(harness.root.querySelector('b')).toBeNull();
    failEdit = false;
    safety.value = 'sfw_only';
    safety.dispatchEvent(new Event('change', { bubbles: true }));
    await settle();
    expect(safety.value).toBe('sfw_only');
    expect(safety.disabled).toBe(false);
  } finally {
    harness.root.remove();
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
  ['missingModelLookupEnabled', 'Look up missing models'],
  ['downloadsEnabled', 'Download missing CivitAI models'],
] as const)(
  'persists %s independently of automatic metadata and thumbnails',
  async (key, label) => {
    let server = initial;
    const patches: unknown[] = [];
    const harness = mount(async (_path, options) => {
      if (options?.method === 'PATCH') {
        const payload: unknown = JSON.parse(String(options.body));
        patches.push(payload);
        if (!isRecord(payload) || payload[key] !== false)
          throw new Error('Invalid independent edit.');
        server = { ...server, [key]: false };
      }
      return response(server);
    });
    try {
      await settle();
      const checkbox = Array.from(harness.root.querySelectorAll('fixture-provider-checkbox')).find(
        (control) => control.getAttribute('aria-label') === label,
      );
      if (!(checkbox instanceof FixtureCheckbox)) throw new Error('Missing acquisition setting.');
      expect(checkbox.checked).toBe(true);
      expect(checkbox.textContent).toBe('');
      expect(checkbox.nextElementSibling?.textContent).toBe('Enabled');
      checkbox.checked = false;
      checkbox.dispatchEvent(new Event('change', { bubbles: true }));
      await settle();
      expect(patches).toEqual([{ [key]: false }]);
      expect(checkbox.checked).toBe(false);
      expect(checkbox.disabled).toBe(false);
      expect(server.metadataLookupEnabled).toBe(false);
      expect(server.thumbnailDownloadsEnabled).toBe(true);
      expect(server.thumbnailSafety).toBe('allow_soft');
      expect(harness.report).not.toHaveBeenCalled();
    } finally {
      harness.root.remove();
    }
  },
);
