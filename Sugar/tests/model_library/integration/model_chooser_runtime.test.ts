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
/** Prove the picker popup can be opened on its own to choose one model at a point. */
import { afterEach, beforeAll, expect, test } from '@jest/globals';
import { nextTick } from 'vue';
import { openModelChooser } from '../../../frontend/comfyui/ui/model_library/vue/ModelChooserVueRuntime.js';
import type { ModelCatalogItem } from '../../../frontend/comfyui/ui/model_library/ModelCatalog.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import { ModelFixtureResizeObserver } from '../support/resize_observer.js';
import { liveModels } from '../support/live_models.js';

beforeAll(() => {
  Object.defineProperty(window, 'ResizeObserver', {
    value: ModelFixtureResizeObserver,
    configurable: true,
  });
});
afterEach(() => document.body.replaceChildren());

const items = modelPickerChoices([], 'loras', [
  'styles/ink.safetensors',
  'styles/pastel.safetensors',
  'characters/knight.safetensors',
]);

const context = {
  actions: null,
  updates: null,
  feedback: null,
  detectors: null,
  outputs: null,
  detectorDialog: null,
};

/** Open the chooser over the fixture models and collect how it settles. */
function open(models: readonly ModelCatalogItem[] = items) {
  const settled: (ModelCatalogItem | null)[] = [];
  const catalog = liveModels(models);
  const chooser = openModelChooser(
    document,
    {
      items: catalog.models,
      label: 'Search LoRA',
      anchor: { left: 40, top: 40, width: 1, height: 1 },
      context,
      session: null,
    },
    (item) => settled.push(item),
  );
  const input = chooser.popup?.querySelector('input');
  if (!chooser.popup || !input) throw new Error('The chooser did not mount.');
  const tiles = (): string[] =>
    [...document.querySelectorAll<HTMLElement>('.sc-model-tile')].map(
      (tile) => tile.dataset.modelValue ?? '',
    );
  const type = async (text: string): Promise<void> => {
    input.value = text;
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await nextTick();
  };
  const press = async (key: string): Promise<void> => {
    input.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
    await nextTick();
  };
  return { chooser, input, settled, tiles, type, press, catalog };
}

test('opens as the picker popup with a named search box, folders and every model', async () => {
  const f = open();
  await nextTick();
  expect(f.chooser.popup?.classList.contains('sc-model-popup')).toBe(true);
  expect(f.chooser.popup?.getAttribute('aria-label')).toBe('Search LoRA');
  expect(f.input.placeholder).toBe('Search LoRA');
  expect(document.activeElement).toBe(f.input);
  expect(f.tiles()).toEqual(items.map((item) => item.value));
  expect(
    [...document.querySelectorAll('.sc-model-routes button')].map((button) => button.textContent),
  ).toEqual(['characters', 'styles']);
  f.chooser.dismiss();
});

test('search narrows the wall and Enter chooses the current model', async () => {
  const f = open();
  await f.type('pastel');
  expect(f.tiles()).toEqual(['styles/pastel.safetensors']);
  await f.press('Enter');
  expect(f.settled.map((item) => item?.value)).toEqual(['styles/pastel.safetensors']);
  // The popup is gone before the caller hears of the choice.
  expect(document.querySelector('.sc-model-popup')).toBeNull();
});

test('a folder opens without choosing, arrows move the current tile, and a click chooses', async () => {
  const f = open();
  await nextTick();
  const styles = [...document.querySelectorAll<HTMLElement>('.sc-model-routes button')].find(
    (button) => button.textContent === 'styles',
  );
  styles?.click();
  await nextTick();
  expect(f.settled).toEqual([]);
  expect(f.tiles()).toEqual(['styles/ink.safetensors', 'styles/pastel.safetensors']);
  await f.press('ArrowRight');
  expect(document.querySelector('.sc-model-active')?.getAttribute('data-model-value')).toBe(
    'styles/pastel.safetensors',
  );
  document.querySelector<HTMLElement>('.sc-model-tile')?.click();
  expect(f.settled.map((item) => item?.value)).toEqual(['styles/ink.safetensors']);
});

test('Escape, a press elsewhere and an explicit dismissal each settle once with no model', async () => {
  const escaped = open();
  await escaped.press('Escape');
  expect(escaped.settled).toEqual([null]);

  const outside = open();
  await nextTick();
  // A press inside the popup keeps it open.
  outside.input.dispatchEvent(new Event('pointerdown', { bubbles: true }));
  expect(outside.settled).toEqual([]);
  document.body.dispatchEvent(new Event('pointerdown', { bubbles: true }));
  expect(outside.settled).toEqual([null]);

  const dismissed = open();
  dismissed.chooser.dismiss();
  dismissed.chooser.dismiss();
  expect(dismissed.settled).toEqual([null]);
  expect(document.querySelector('.sc-model-popup')).toBeNull();
});

test('right-clicking a card opens the same model menu as the model picker, and choosing nothing', async () => {
  const linked = items.map((item) => ({
    ...item,
    provider: 'civitai' as const,
    providerUrl: 'https://civitai.com/models/1',
  }));
  const f = open(linked);
  await nextTick();
  const tile = document.querySelector<HTMLElement>('.sc-model-tile')!;
  const event = new MouseEvent('contextmenu', {
    bubbles: true,
    cancelable: true,
    clientX: 60,
    clientY: 60,
  });
  tile.dispatchEvent(event);
  await nextTick();
  expect(event.defaultPrevented).toBe(true);
  const menu = document.querySelector('[data-sc-floating-surface]');
  expect(menu).not.toBeNull();
  expect(menu!.textContent).toContain('Go to CivitAI page');
  // Pressing in the card's menu is not a press outside the chooser.
  menu!.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true }));
  expect(f.settled).toEqual([]);
  f.chooser.dismiss();
});

test('an open chooser shows what the catalog publishes, without being reopened', async () => {
  const f = open();
  await nextTick();
  const [first, ...rest] = items;
  if (!first) throw new Error('The fixture has no models.');
  const tile = (): HTMLElement | null =>
    document.querySelector<HTMLElement>(`.sc-model-tile[data-model-value="${first.value}"]`);
  expect(tile()?.textContent).toContain('No Thumbnail');
  f.catalog.publish([
    { ...first, image: { state: 'not_found', blurred: false, reveal: [] } },
    ...rest,
  ]);
  await nextTick();
  expect(tile()?.textContent).toContain('Not Found');
  expect(tile()?.textContent).not.toContain('No Thumbnail');
  f.catalog.publish([
    { ...first, image: { state: 'retrying', blurred: false, reveal: [] } },
    ...rest,
  ]);
  await nextTick();
  expect(tile()?.querySelector('.sc-model-blank-flash')?.textContent).toBe('Retrying…');
  f.chooser.dismiss();
  // A closed chooser no longer follows the catalog.
  expect(f.catalog.watchers()).toBe(0);
});

test('a chooser cannot be handed the catalog as it was when it opened', () => {
  /** Never run: the compiler is the proof. A chooser takes models that follow the catalog. */
  const frozen = (models: readonly ModelCatalogItem[]): void => {
    openModelChooser(
      document,
      {
        // @ts-expect-error An array is the catalog as it was; a mounted surface must follow it.
        items: models,
        label: 'Search LoRA',
        anchor: { left: 0, top: 0, width: 1, height: 1 },
        context,
        session: null,
      },
      () => undefined,
    );
  };
  expect(typeof frozen).toBe('function');
});
