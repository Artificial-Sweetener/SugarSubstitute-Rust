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
/** Prove conditional navigation rows and overflow hit space follow the actual route contents. */
import { createApp, h, nextTick, ref } from 'vue';
import { jest } from '@jest/globals';
import { ModelPickerPopupContent } from '../../../frontend/comfyui/ui/model_library/vue/ModelPickerPopupContent.js';
import { ModelPickerRouteRow } from '../../../frontend/comfyui/ui/model_library/vue/ModelPickerRouteRow.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import { modelPickerCss } from '../../../frontend/comfyui/ui/model_library/ModelPickerStyles.js';
import { ModelFixtureResizeObserver } from '../support/resize_observer.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test('omits empty navigation and keeps a compact Show all row when the filter is the only control', async () => {
  Object.defineProperty(window, 'ResizeObserver', {
    value: ModelFixtureResizeObserver,
    configurable: true,
  });
  const root = document.createElement('div');
  const style = document.createElement('style');
  style.textContent = modelPickerCss;
  document.body.append(style, root);
  const items = modelPickerChoices([], 'checkpoints', ['alpha.safetensors']);
  const label = ref('');
  const unfiltered = ref(false);
  const toggle = jest.fn();
  const app = createApp({
    /** Render real popup navigation independently of model discovery or host state. */
    render: () =>
      h(ModelPickerPopupContent, {
        items,
        visible: items,
        folder: '',
        selected: 'alpha.safetensors',
        active: 0,
        optionPrefix: 'routes',
        compatibilityLabel: label.value,
        unfiltered: unfiltered.value,
        onToggleFilter: toggle,
      }),
  });
  app.mount(root);
  try {
    expect(root.querySelector('.sc-model-routes')).toBeNull();
    label.value = 'SDXL';
    await nextTick();
    expect(root.querySelector('.sc-model-breadcrumbs')).toBeNull();
    expect(root.querySelectorAll('.sc-model-folder-strip')).toHaveLength(1);
    expect(root.querySelector('.sc-model-route-overflow')).toBeNull();
    expect(root.querySelector('.sc-model-child-folder')).toBeNull();
    const button = root.querySelector<HTMLButtonElement>('.sc-model-filter-toggle');
    expect(button?.textContent).toBe('Show all');
    if (!button) throw new Error('Missing filter button.');
    expect(button.dataset.widthLabel).toBe('Show only SDXL');
    expect(getComputedStyle(button).display).toBe('grid');
    expect(getComputedStyle(button).gridTemplateColumns).toBe('16px auto');
    const filterLabel = button.querySelector('.sc-model-filter-label');
    if (!filterLabel) throw new Error('Missing centered filter label.');
    expect(getComputedStyle(filterLabel).textAlign).toBe('center');
    expect(getComputedStyle(filterLabel).gridArea).toBe('1/2');
    const icon = button?.querySelector('.sc-model-filter-icon');
    if (!icon) throw new Error('Missing filter icon.');
    expect(icon.getAttribute('aria-hidden')).toBe('true');
    expect(getComputedStyle(icon).gridArea).toBe('1/1');
    expect(getComputedStyle(icon).mask).toContain('/eye.svg');
    button?.click();
    expect(toggle).toHaveBeenCalledTimes(1);
    unfiltered.value = true;
    await nextTick();
    expect(button?.textContent).toBe('Show only SDXL');
    expect(button.dataset.widthLabel).toBe('Show only SDXL');
    expect(getComputedStyle(icon).mask).toContain('/eye-off.svg');
    unfiltered.value = false;
    await nextTick();
    expect(button?.textContent).toBe('Show all');
    expect(button.dataset.widthLabel).toBe('Show only SDXL');
    expect(getComputedStyle(icon).mask).toContain('/eye.svg');
    label.value = '';
    await nextTick();
    expect(root.querySelector('.sc-model-routes')).toBeNull();
  } finally {
    app.unmount();
  }
});

test('reserves scrollbar hit space only during overflow and releases the observer on unmount', async () => {
  let resize = (): void => {
    throw new Error('Route observer did not mount.');
  };
  const disconnect = jest.fn();
  class RouteObserver implements ResizeObserver {
    /** Expose a deterministic browser resize boundary. */
    constructor(callback: ResizeObserverCallback) {
      resize = () => callback([], this);
    }
    /** Accept the route as the measured element. */
    observe(_target: Element): void {}
    /** Release a single element if requested. */
    unobserve(_target: Element): void {}
    /** Record lifetime disposal. */
    disconnect(): void {
      disconnect();
    }
  }
  Object.defineProperty(window, 'ResizeObserver', { value: RouteObserver, configurable: true });
  let width = 120;
  let contents = 120;
  jest.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockImplementation(() => width);
  jest.spyOn(HTMLElement.prototype, 'scrollWidth', 'get').mockImplementation(() => contents);
  const root = document.createElement('div');
  const style = document.createElement('style');
  style.textContent = modelPickerCss;
  document.body.append(style, root);
  const label = ref('Folder');
  const app = createApp({
    /** Update slot contents through the same render boundary as folder navigation. */
    render: () => h(ModelPickerRouteRow, { rowClass: 'sc-model-folder-strip' }, () => label.value),
  });
  app.mount(root);
  try {
    const row = root.querySelector('.sc-model-folder-strip');
    if (!row) throw new Error('Missing route row.');
    expect(getComputedStyle(row).minHeight).toBe('24px');
    expect(getComputedStyle(row).paddingBottom).toBe('0px');
    contents = 200;
    label.value = 'A longer set of folders';
    await nextTick();
    await nextTick();
    expect(row.classList.contains('sc-model-route-overflow')).toBe(true);
    expect(getComputedStyle(row).minHeight).toBe('36px');
    expect(getComputedStyle(row).paddingBottom).toBe('12px');
    width = 220;
    resize();
    await nextTick();
    expect(row.classList.contains('sc-model-route-overflow')).toBe(false);
    expect(getComputedStyle(row).paddingBottom).toBe('0px');
  } finally {
    app.unmount();
  }
  expect(disconnect).toHaveBeenCalledTimes(1);
});

test('shares the filter with child folders or leaf breadcrumbs and gives it a row only without navigation', async () => {
  Object.defineProperty(window, 'ResizeObserver', {
    value: ModelFixtureResizeObserver,
    configurable: true,
  });
  const root = document.createElement('div');
  document.body.append(root);
  const items = ref(
    modelPickerChoices([], 'checkpoints', [
      'Illustrious/root.safetensors',
      'Illustrious/Characters/portrait.safetensors',
      'Pony/other.safetensors',
    ]),
  );
  const folder = ref('');
  const unfiltered = ref(false);
  const app = createApp({
    /** Follow navigation and filter events through the real popup component. */
    render: () =>
      h(ModelPickerPopupContent, {
        items: items.value,
        visible: items.value,
        folder: folder.value,
        selected: '',
        active: 0,
        optionPrefix: 'shared-filter',
        compatibilityLabel: 'SDXL',
        unfiltered: unfiltered.value,
        onNavigate: (path: string) => {
          folder.value = path;
        },
        onToggleFilter: () => {
          unfiltered.value = !unfiltered.value;
        },
      }),
  });
  app.mount(root);
  /** Click a route's actual control without replacing component events. */
  const click = (label: string): void => {
    const button = [...root.querySelectorAll<HTMLButtonElement>('.sc-model-routes button')].find(
      (entry) => entry.textContent === label,
    );
    if (!button) throw new Error(`Missing ${label} navigation control.`);
    button.click();
  };
  try {
    expect(root.querySelectorAll('.sc-model-routes > div')).toHaveLength(1);
    expect(root.querySelector('.sc-model-folder-strip .sc-model-filter-toggle')).not.toBeNull();
    click('Illustrious');
    await nextTick();
    expect(root.querySelectorAll('.sc-model-routes > div')).toHaveLength(2);
    expect(root.querySelector('.sc-model-breadcrumbs')?.textContent).toBe('All›Illustrious');
    expect(root.querySelector('.sc-model-folder-strip')?.textContent).toBe('CharactersShow all');
    click('Characters');
    await nextTick();
    expect(root.querySelectorAll('.sc-model-routes > div')).toHaveLength(1);
    expect(root.querySelector('.sc-model-folder-strip')).toBeNull();
    expect(root.querySelector('.sc-model-breadcrumbs')?.textContent).toBe(
      'All›Illustrious›CharactersShow all',
    );
    click('Show all');
    await nextTick();
    expect(
      root
        .querySelector('.sc-model-breadcrumbs .sc-model-filter-toggle')
        ?.getAttribute('aria-pressed'),
    ).toBe('true');
    expect(root.querySelectorAll('.sc-model-routes > div')).toHaveLength(1);
    click('All');
    await nextTick();
    expect(root.querySelector('.sc-model-breadcrumbs')).toBeNull();
    expect(root.querySelector('.sc-model-folder-strip')?.textContent).toBe(
      'IllustriousPonyShow only SDXL',
    );
    items.value = modelPickerChoices([], 'checkpoints', ['alpha.safetensors']);
    await nextTick();
    expect(root.querySelectorAll('.sc-model-routes > div')).toHaveLength(1);
    expect(root.querySelector('.sc-model-folder-strip')?.textContent).toBe('Show only SDXL');
    expect(root.querySelector('.sc-model-filter-toggle')?.getAttribute('title')).toBe(
      'Show only SDXL',
    );
    expect(root.querySelector('.sc-model-child-folder')).toBeNull();
  } finally {
    app.unmount();
  }
});
