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
/** Prove model dialogs cancel owned work and nested credential dismissal preserves selection. */
import { jest, test, expect, beforeEach, afterEach } from '@jest/globals';
import { nextTick } from 'vue';
import { ModelDiscoveryDialog } from '../../../frontend/comfyui/ui/model_library/ModelDiscoveryDialog.js';
import { ModelDetectorDialog } from '../../../frontend/comfyui/ui/model_library/ModelDetectorDialog.js';
import { ModelVersionDialog } from '../../../frontend/comfyui/ui/model_library/ModelVersionDialog.js';
import { CivitaiKeyDialog } from '../../../frontend/comfyui/ui/model_library/CivitaiKeyDialog.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import { discoveryPorts } from '../support/model_discovery.js';
import { ModelFixtureResizeObserver } from '../support/resize_observer.js';
import type { ModelPickerActions } from '../../../frontend/comfyui/ui/model_library/ModelPickerActions.js';

beforeEach(() => {
  Object.defineProperty(window, 'ResizeObserver', {
    value: ModelFixtureResizeObserver,
    configurable: true,
  });
});
afterEach(() => document.body.replaceChildren());

/** Drain view updates without fetching remote providers. */
async function settle(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  await nextTick();
}

/** Activate a specific owned dismissal route through its live DOM. */
function dismiss(overlay: HTMLElement, action: 'close' | 'outside'): void {
  if (action === 'outside') overlay.click();
  else {
    const button = overlay.querySelector<HTMLButtonElement>('.sugarcubes-modal__close');
    if (!button) throw new Error('Missing model header close action');
    button.click();
  }
}

test.each(['close', 'outside'] as const)(
  'model dialogs: %s cancels without saving or applying values',
  async (action) => {
    const ports = discoveryPorts();
    const item = modelPickerChoices([], 'ultralytics', ['bbox/face.pt'])[0];
    if (!item) throw new Error('Missing model fixture');
    const select = jest.fn();
    const assign = jest.fn(async () => undefined);
    const releases = jest.fn<ModelPickerActions['releases']>(() => new Promise(() => undefined));
    const acquire = jest.fn<ModelPickerActions['acquire']>();
    const dialogs = [
      new ModelDiscoveryDialog(
        document,
        'upscale_models',
        '',
        ports.discovery,
        ports.credentials,
        select,
      ),
      new ModelDetectorDialog(document, item, {
        resolve: async () => [],
        library: async () => [],
        assign,
      }),
      new CivitaiKeyDialog(document, ports.credentials),
      new ModelVersionDialog(
        document,
        item,
        {
          credentials: ports.credentials,
          classifyContent: async () => undefined,
          refreshMetadata: async () => undefined,
          replaceThumbnail: async () => undefined,
          releases,
          acquire,
        },
        {
          current: () => item.value,
          select,
          download: async () => '',
          useDownload: async () => undefined,
        },
        jest.fn(),
      ),
    ];
    for (const dialog of dialogs) {
      const result = dialog.open();
      try {
        await settle();
        const overlay = document.querySelector<HTMLElement>('.sugarcubes-modal-overlay.is-visible');
        if (!overlay) throw new Error('Missing model dialog');
        dismiss(overlay, action);
        await result;
        expect(overlay.isConnected).toBe(false);
      } finally {
        dialog.close();
        await result;
      }
    }
    expect(ports.discovery.release).toHaveBeenCalledTimes(1);
    expect(ports.credentials.save).not.toHaveBeenCalled();
    expect(ports.discovery.acquire).not.toHaveBeenCalled();
    expect(assign).not.toHaveBeenCalled();
    expect(acquire).not.toHaveBeenCalled();
    expect(select).not.toHaveBeenCalled();
    expect(releases.mock.calls[0]?.[1].aborted).toBe(true);
  },
);

test.each(['close', 'outside'] as const)(
  'contained credential: %s returns to the selected model',
  async (action) => {
    const ports = discoveryPorts();
    const dialog = new ModelDiscoveryDialog(
      document,
      'upscale_models',
      '',
      ports.discovery,
      ports.credentials,
      jest.fn(),
    );
    const result = dialog.open();
    try {
      await settle();
      document.querySelector<HTMLButtonElement>('.sc-model-discovery-portrait')?.click();
      await nextTick();
      document
        .querySelector('.sc-model-discovery-card')
        ?.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
      await nextTick();
      const source = [...document.querySelectorAll<HTMLButtonElement>('button')].find(
        (el) => el.textContent === 'Download from CivitAI',
      );
      if (!source) throw new Error('Missing protected provider');
      source.click();
      await nextTick();
      const accept = document.querySelector<HTMLButtonElement>(
        '.sc-model-discovery-modal .sugarcubes-modal__footer-actions button:last-child',
      );
      if (!accept) throw new Error('Missing credential action');
      accept.click();
      await nextTick();
      const layer = document.querySelector<HTMLElement>('.sc-model-credential-layer');
      if (!layer) throw new Error('Missing credential layer');
      const input = layer.querySelector<HTMLInputElement>('input');
      if (!input) throw new Error('Missing key input');
      input.value = 'unsaved-fixture-key';
      input.dispatchEvent(new Event('input', { bubbles: true }));
      await nextTick();
      document.querySelector<HTMLElement>('.sc-model-discovery-overlay')?.click();
      expect(document.querySelector('.sc-model-credential-layer')).toBe(layer);
      dismiss(layer, action);
      await nextTick();
      expect(document.querySelector('.sc-model-credential-layer')).toBeNull();
      expect(document.querySelector('.sc-model-discovery-overlay.is-visible')).not.toBeNull();
      expect(document.querySelector('[role=checkbox]')?.getAttribute('aria-checked')).toBe('true');
      expect(document.activeElement).toBe(accept);
      expect(ports.credentials.save).not.toHaveBeenCalled();
      expect(ports.discovery.acquire).not.toHaveBeenCalled();
    } finally {
      dialog.close();
      await result;
    }
  },
);
