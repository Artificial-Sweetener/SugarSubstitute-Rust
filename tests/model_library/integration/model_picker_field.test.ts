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
/** Prove editable model fields commit legal selections and cancel temporary search safely. */
import { nextTick } from 'vue';
import { jest } from '@jest/globals';
import { mountModelPicker } from '../../../frontend/comfyui/ui/model_library/vue/ModelPickerVueRuntime.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import type { ModelPickerBinding } from '../../../frontend/comfyui/ui/model_library/ModelPickerBinding.js';
import { modelPickerCss } from '../../../frontend/comfyui/ui/model_library/ModelPickerStyles.js';
import { fluentAppearanceCss } from '../../../frontend/comfyui/ui/surface/appearance/FluentAppearanceStyles.js';

import { ModelFixtureResizeObserver } from '../support/resize_observer.js';

/** Build one native-value boundary whose writes can be observed independently of search text. */
function fixture(extraOptions: readonly string[] = []) {
  let value = 'Anima\\alpha.safetensors';
  const writes: string[] = [];
  const listeners = new Set<() => void>();
  const container = document.createElement('div');
  document.body.append(container);
  const options = [value, 'Anima\\beta.safetensors', 'Other\\gamma.safetensors', ...extraOptions];
  const binding: ModelPickerBinding = {
    label: 'lora_name',
    kind: 'loras',
    options,
    anchor: container,
    readValue: () => value,
    disabled: () => false,
    writeValue: (next) => {
      value = next;
      writes.push(next);
      for (const listener of listeners) listener();
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
  const items = modelPickerChoices([], 'loras', options).map((item) =>
    item.title === 'beta' ? { ...item, title: '<b>beta</b>' } : item,
  );
  const picker = mountModelPicker(container, binding, items);
  const input = container.querySelector('input');
  if (!input) throw new Error('The fixture model field did not mount.');
  return { container, input, binding, picker, writes, listeners };
}

beforeAll(() => {
  Object.defineProperty(window, 'ResizeObserver', {
    value: ModelFixtureResizeObserver,
    configurable: true,
  });
});
afterEach(() => {
  document.body.replaceChildren();
});

test.each([true, false])(
  'shares regular Fluent combo styling in dark=%s through thumbnail changes',
  async (dark) => {
    const field = fixture();
    field.container.dataset.scFluentSurface = '';
    const style = document.createElement('style');
    style.textContent = modelPickerCss + fluentAppearanceCss(dark);
    field.container.append(style);
    const regular = document.createElement('div');
    regular.dataset.scFluentComboFrame = '';
    field.container.append(regular);
    const model = field.container.querySelector('.sc-model-field');
    if (!(model instanceof HTMLElement)) throw new Error('No plain model field mounted.');
    expect(model.querySelector('.sc-model-banner')).toBeNull();
    for (const property of [
      'height',
      'min-height',
      'border-radius',
      'background',
      'border',
      'border-bottom-color',
    ])
      expect(getComputedStyle(model).getPropertyValue(property)).toBe(
        getComputedStyle(regular).getPropertyValue(property),
      );
    expect(getComputedStyle(model).height).toBe('33px');
    field.picker.update(
      modelPickerChoices([], 'loras', field.binding.options).map((item) => ({
        ...item,
        thumbnails: [
          { assetId: 'fixture', role: 'banner', width: 768, height: 160, url: '/banner' },
        ],
      })),
    );
    await nextTick();
    const banner = model.querySelector('.sc-model-banner');
    if (!(banner instanceof HTMLImageElement)) throw new Error('No thumbnail banner mounted.');
    expect(getComputedStyle(model).height).toBe('32px');
    banner.dispatchEvent(new Event('error'));
    await nextTick();
    expect(model.querySelector('.sc-model-banner')).toBeNull();
    expect(getComputedStyle(model).height).toBe('33px');
    field.picker.dispose();
  },
);

test.each([
  {
    name: 'narrow',
    width: 200,
    height: 400,
    top: 90,
    fieldHeight: 32,
    expected: [8, 122, 184, 270],
  },
  { name: 'short', width: 572, height: 100, top: 40, fieldHeight: 32, expected: [8, 8, 556, 32] },
  { name: 'tiny', width: 12, height: 12, top: 5, fieldHeight: 2, expected: [0, 7, 12, 5] },
])(
  'keeps the attached picker inside a $name viewport',
  async ({ width, height, top, fieldHeight, expected }) => {
    const field = fixture();
    const surface = field.container.querySelector('.sc-model-field');
    if (!(surface instanceof HTMLElement)) throw new Error('No model field surface mounted.');
    jest.replaceProperty(window, 'innerWidth', width);
    jest.replaceProperty(window, 'innerHeight', height);
    try {
      jest.spyOn(surface, 'getBoundingClientRect').mockReturnValue({
        x: 1,
        y: top,
        left: 1,
        right: width - 1,
        top,
        bottom: top + fieldHeight,
        width: width - 2,
        height: fieldHeight,
        toJSON: () => ({}),
      });
      field.input.click();
      await nextTick();
      const popup = document.querySelector('.sc-model-popup');
      if (!(popup instanceof HTMLElement)) throw new Error('No model popup mounted.');
      expect(
        [popup.style.left, popup.style.top, popup.style.width, popup.style.height].map(parseFloat),
      ).toEqual(expected);
      expect(field.writes).toEqual([]);
      field.input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
      await nextTick();
      expect(document.querySelector('.sc-model-popup')).toBeNull();
      expect(field.binding.readValue()).toBe('Anima\\alpha.safetensors');
    } finally {
      field.picker.dispose();
      jest.restoreAllMocks();
    }
  },
);

test('opens at the root and separates breadcrumbs from available child folders', async () => {
  const field = fixture();
  field.input.click();
  await nextTick();
  const routes = document.querySelector('.sc-model-routes');
  if (!routes) throw new Error('No folder navigation mounted.');
  expect(routes.querySelector('.sc-model-breadcrumbs')).toBeNull();
  expect(
    Array.from(routes.querySelectorAll('.sc-model-child-folder'), (button) => button.textContent),
  ).toEqual(['Anima', 'Other']);
  expect(document.querySelectorAll('.sc-model-tile')).toHaveLength(3);
  const anima = routes.querySelector('button');
  if (!(anima instanceof HTMLElement)) throw new Error('No Anima folder button.');
  anima.click();
  await nextTick();
  expect(
    Array.from(
      routes.querySelector('.sc-model-breadcrumbs')?.children ?? [],
      (button) => button.textContent,
    ),
  ).toEqual(['All', '›', 'Anima']);
  expect(document.querySelectorAll('.sc-model-tile')).toHaveLength(2);
  expect(field.writes).toEqual([]);
  const all = routes.querySelector('button');
  if (!(all instanceof HTMLElement)) throw new Error('No root breadcrumb.');
  all.click();
  await nextTick();
  expect(routes.querySelector('.sc-model-breadcrumbs')).toBeNull();
  expect(
    Array.from(routes.querySelectorAll('.sc-model-child-folder'), (button) => button.textContent),
  ).toEqual(['Anima', 'Other']);
  expect(document.querySelectorAll('.sc-model-tile')).toHaveLength(3);
  const reopenedFolder = routes.querySelector('button');
  if (!(reopenedFolder instanceof HTMLElement)) throw new Error('No root folder button.');
  reopenedFolder.click();
  await nextTick();
  field.input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  await nextTick();
  field.input.click();
  await nextTick();
  expect(document.querySelectorAll('.sc-model-tile')).toHaveLength(3);
  expect(field.binding.readValue()).toBe('Anima\\alpha.safetensors');
  expect(field.writes).toEqual([]);
  field.picker.dispose();
});

test('drills into nested folders on a separate strip and returns using breadcrumbs without writes', async () => {
  const field = fixture([
    'Anima/Portrait/Characters/delta.safetensors',
    'Anima/Styles/epsilon.safetensors',
  ]);
  try {
    field.input.click();
    await nextTick();
    /** Navigate a visible child route without committing a model. */
    const clickFolder = (label: string): void => {
      const button = [
        ...document.querySelectorAll<HTMLButtonElement>('.sc-model-child-folder'),
      ].find((entry) => entry.textContent === label);
      if (!button) throw new Error(`Missing child folder ${label}.`);
      button.click();
    };
    clickFolder('Anima');
    await nextTick();
    const breadcrumbs = document.querySelector('.sc-model-breadcrumbs');
    const strip = document.querySelector('.sc-model-folder-strip');
    if (!breadcrumbs || !strip) throw new Error('Missing separate folder navigation rows.');
    expect(breadcrumbs?.textContent).toBe('All›Anima');
    expect(
      [...(strip?.querySelectorAll('button') ?? [])].map((button) => button.textContent),
    ).toEqual(['Portrait', 'Styles']);
    expect(breadcrumbs?.parentElement).toBe(strip?.parentElement);
    expect(breadcrumbs?.contains(strip)).toBe(false);
    clickFolder('Portrait');
    await nextTick();
    expect(document.querySelector('.sc-model-breadcrumbs')?.textContent).toBe('All›Anima›Portrait');
    expect(document.querySelector('.sc-model-folder-strip')?.textContent).toBe('Characters');
    clickFolder('Characters');
    await nextTick();
    expect(document.querySelector('.sc-model-breadcrumbs')?.textContent).toBe(
      'All›Anima›Portrait›Characters',
    );
    expect(document.querySelector('.sc-model-folder-strip')).toBeNull();
    const all = document.querySelector<HTMLButtonElement>('.sc-model-breadcrumbs button');
    all?.click();
    await nextTick();
    expect(document.querySelector('.sc-model-breadcrumbs')).toBeNull();
    expect(document.querySelectorAll('.sc-model-tile')).toHaveLength(5);
    expect(field.writes).toEqual([]);
    expect(field.binding.readValue()).toBe('Anima\\alpha.safetensors');
  } finally {
    field.picker.dispose();
  }
});

test.each([false, true])(
  'bounds the picker to its available control width in masonry=%s',
  (masonry) => {
    const field = fixture();
    if (masonry) field.container.className = 'sugarcubes-workflow-masonry';
    const style = document.createElement('style');
    style.textContent = modelPickerCss;
    document.head.append(style);
    try {
      const editor = field.container.querySelector('.sc-model-field');
      if (!editor) throw new Error('The fixture model editor did not mount.');
      const geometry = getComputedStyle(editor);
      expect(geometry.minWidth).toBe('0');
      expect(geometry.maxWidth).toBe('100%');
      expect(geometry.boxSizing).toBe('border-box');
    } finally {
      style.remove();
      field.picker.dispose();
    }
  },
);

test('keeps the reference frame inset and caption spacing inside the attached gallery', async () => {
  const field = fixture();
  const style = document.createElement('style');
  style.textContent = modelPickerCss;
  document.head.append(style);
  try {
    field.input.click();
    await nextTick();
    const popup = document.querySelector('.sc-model-popup');
    const caption = document.querySelector('.sc-model-caption');
    if (!popup || !caption) throw new Error('Missing attached gallery geometry.');
    expect(getComputedStyle(popup).padding).toBe('10px');
    expect(getComputedStyle(caption).inset).toBe('auto 8px 7px');
    expect(getComputedStyle(caption).gap).toBe('2px');
    const route = popup.querySelector('.sc-model-routes button');
    if (!route) throw new Error('Missing route label.');
    expect(getComputedStyle(route).lineHeight).toBe('19px');
    expect(getComputedStyle(route).height).toBe('24px');
    expect(getComputedStyle(route).margin).toBe('0px');
    const routes = popup.querySelector('.sc-model-routes');
    if (!routes) throw new Error('Missing compact route row.');
    expect(getComputedStyle(routes).paddingBottom).toBe('4px');
    const strip = popup.querySelector('.sc-model-folder-strip');
    if (!strip) throw new Error('Missing horizontal folder strip.');
    expect(getComputedStyle(strip).paddingBottom).toBe('0px');
    expect(getComputedStyle(strip).minHeight).toBe('24px');
  } finally {
    style.remove();
    field.picker.dispose();
  }
});

test('searches in the field, cancels without writes, then selects an exact native value', async () => {
  const field = fixture();
  field.input.click();
  await nextTick();
  expect(field.input.readOnly).toBe(false);
  field.input.value = 'beta';
  field.input.dispatchEvent(new Event('input', { bubbles: true }));
  await nextTick();
  expect(field.writes).toEqual([]);
  expect(document.querySelectorAll('.sc-model-popup input')).toHaveLength(0);
  field.input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  await nextTick();
  expect(document.querySelector('.sc-model-popup')).toBeNull();
  expect(field.binding.readValue()).toBe('Anima\\alpha.safetensors');
  field.input.click();
  await nextTick();
  field.input.value = 'beta';
  field.input.dispatchEvent(new Event('input', { bubbles: true }));
  await nextTick();
  expect(document.querySelector('.sc-model-popup b')).toBeNull();
  expect(document.querySelector('.sc-model-popup')?.textContent).toContain('<b>beta</b>');
  field.input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  await nextTick();
  expect(field.writes).toEqual(['Anima\\beta.safetensors']);
  field.picker.dispose();
  expect(field.listeners.size).toBe(0);
});

test('isolates pointer gestures and removes its popup on unmount', async () => {
  const field = fixture();
  const bubbling = jest.fn();
  document.body.addEventListener('pointerdown', bubbling);
  field.input.dispatchEvent(new Event('pointerdown', { bubbles: true }));
  expect(bubbling).not.toHaveBeenCalled();
  field.input.click();
  await nextTick();
  expect(document.querySelector('.sc-model-popup')).not.toBeNull();
  field.picker.dispose();
  await nextTick();
  expect(document.querySelector('.sc-model-popup')).toBeNull();
  document.body.removeEventListener('pointerdown', bubbling);
});

test('completes the arrow-key highlight from an empty search without committing until Enter', async () => {
  const field = fixture(['Anima\\alpine.safetensors']);
  field.input.click();
  await nextTick();
  expect(field.input.value).toBe('');
  expect(document.querySelector('.sc-model-completion')?.textContent).toBe('alpha');
  field.input.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  await nextTick();
  expect(document.querySelector('.sc-model-completion')?.textContent).toBe('beta');
  field.input.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
  await nextTick();
  expect(document.querySelector('.sc-model-completion')?.textContent).toBe('alpha');
  field.input.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  await nextTick();
  field.input.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true }),
  );
  await nextTick();
  expect(field.input.value).toBe('beta');
  expect(field.writes).toEqual([]);
  field.input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  await nextTick();
  expect(field.writes).toEqual(['Anima\\beta.safetensors']);
  field.picker.dispose();
});

test('completes short substring searches against the arrow-key highlight', async () => {
  const field = fixture(['Anima\\waiAniFlux_v10ForFP8.safetensors', 'Anima\\flux1.safetensors']);
  field.input.click();
  await nextTick();
  field.input.value = 'fl';
  field.input.dispatchEvent(new Event('input', { bubbles: true }));
  await nextTick();
  expect(document.querySelector('.sc-model-completion')?.textContent).toBe('flux_v10ForFP8');
  field.input.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  await nextTick();
  expect(document.querySelector('.sc-model-completion')?.textContent).toBe('flux1');
  field.input.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
  await nextTick();
  field.input.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true }),
  );
  await nextTick();
  expect(field.input.value).toBe('flux_v10ForFP8');
  expect(field.writes).toEqual([]);
  field.input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  await nextTick();
  expect(field.writes).toEqual(['Anima\\waiAniFlux_v10ForFP8.safetensors']);
  field.picker.dispose();
});

test('follows keyboard and pointer highlights in ghost completion without changing the host value', async () => {
  const field = fixture(['Anima\\alpine.safetensors']);
  field.input.click();
  await nextTick();
  field.input.value = 'al';
  field.input.dispatchEvent(new Event('input', { bubbles: true }));
  await nextTick();
  expect(document.querySelector('.sc-model-completion')?.textContent).toBe('alpha');
  field.input.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  await nextTick();
  expect(document.querySelector('.sc-model-completion')?.textContent).toBe('alpine');
  document.querySelectorAll('.sc-model-tile')[0]?.dispatchEvent(new Event('pointerenter'));
  await nextTick();
  expect(document.querySelector('.sc-model-completion')?.textContent).toBe('alpha');
  document.querySelectorAll('.sc-model-tile')[1]?.dispatchEvent(new Event('pointerenter'));
  await nextTick();
  field.input.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true }),
  );
  await nextTick();
  expect(field.input.value).toBe('alpine');
  expect(field.writes).toEqual([]);
  field.input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  await nextTick();
  expect(field.writes).toEqual(['Anima\\alpine.safetensors']);
  expect(document.querySelector('.sc-model-completion')).toBeNull();
  field.picker.dispose();
});

test('accepts metadata-only search results without appending the query to the filename', async () => {
  const field = fixture();
  field.picker.update(
    modelPickerChoices([], 'loras', field.binding.options).map((item) => ({
      ...item,
      baseModel: 'Flux',
    })),
  );
  await nextTick();
  field.input.click();
  await nextTick();
  field.input.value = 'fl';
  field.input.dispatchEvent(new Event('input', { bubbles: true }));
  await nextTick();
  field.input.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  await nextTick();
  expect(document.querySelector('.sc-model-completion')?.textContent).toBe('fl → beta');
  field.input.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true }),
  );
  await nextTick();
  expect(field.input.value).toBe('beta');
  expect(field.writes).toEqual([]);
  field.input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  await nextTick();
  expect(field.writes).toEqual(['Anima\\beta.safetensors']);
  field.picker.dispose();
});

test('keeps the highlighted file through Tab when multiple folders contain the same filename', async () => {
  const field = fixture(['Other\\beta.safetensors']);
  field.input.click();
  await nextTick();
  field.input.dispatchEvent(new KeyboardEvent('keydown', { key: 'End', bubbles: true }));
  await nextTick();
  field.input.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true }),
  );
  await nextTick();
  expect(field.input.value).toBe('beta');
  field.input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  await nextTick();
  expect(field.writes).toEqual(['Other\\beta.safetensors']);
  field.picker.dispose();
});
