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
/** Prove the actual Vue cart renders safe text and preserves draft controls across search and reordering. */
import { afterAll, beforeAll, afterEach, expect, jest, test } from '@jest/globals';
import { nextTick } from 'vue';
import { Textfield } from '@spectrum-web-components/textfield/src/Textfield.js';
import type { FinderInputControl } from '../../../frontend/comfyui/ui/surface/appearance/FinderControls.js';
import { createFallbackIconModel } from '../../../frontend/comfyui/ui/core/CubeFallbackIconRenderer.js';
import { mountCubeStackCart } from '../../../frontend/comfyui/ui/picker/vue/CubeStackCartRuntime.js';
import type {
  CubeStackCartViewActions,
  CubeStackCartViewState,
  CubeStackLibraryRow,
} from '../../../frontend/comfyui/ui/picker/CubeStackCartView.js';

const releases: (() => void)[] = [];
const resizeObserverDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'ResizeObserver');

/** Supply the external browser geometry API while cart proofs exercise real Adobe input behavior. */
class CartResizeObserver implements ResizeObserver {
  /** Leave geometry delivery to manual browser proofs. */
  observe(_target: Element): void {}
  /** Release a target without manufacturing geometry changes. */
  unobserve(_target: Element): void {}
  /** Release the fixture observer. */
  disconnect(): void {}
}

beforeAll(() => {
  Object.defineProperty(globalThis, 'ResizeObserver', {
    configurable: true,
    value: CartResizeObserver,
  });
});
afterAll(() => {
  if (resizeObserverDescriptor)
    Object.defineProperty(globalThis, 'ResizeObserver', resizeObserverDescriptor);
  else Reflect.deleteProperty(globalThis, 'ResizeObserver');
});
afterEach(() => {
  for (const release of releases.splice(0)) release();
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test.each(['fluent', 'comfy', 'spectrum'] as const)(
  'shows the same literal model overlay in the %s library and staged cart',
  async (appearance) => {
    const f = fixture();
    const literal = '<b>SDXL</b>';
    f.view.update({
      ...f.state,
      appearance,
      library: f.state.library.map((row) => ({ ...row, targetModel: literal })),
      rows: f.state.rows.map((row) => ({ ...row, targetModel: literal })),
    });
    await nextTick();
    for (const button of [f.libraryButton(), f.cartButton(0)]) {
      expect(
        button.querySelector('.sc-cube-stack-row__icon .sc-cube-stack-row__model')?.textContent,
      ).toBe(literal);
      expect(button.querySelector('b')).toBeNull();
    }
  },
);

test('groups captured artifacts, renders markup literally and stages repeated clicks without native construction', async () => {
  const f = fixture();
  expect(f.element.textContent).toContain('SDXL');
  expect(f.element.textContent).toContain('Start cubes');
  expect(f.element.textContent).toContain('<b>Library cube</b>');
  expect(f.element.querySelector('b')).toBeNull();
  const button = f.libraryButton();
  button.click();
  button.click();
  expect(f.actions.add.mock.calls).toEqual([['artifact'], ['artifact']]);
  expect(f.actions.reorder).not.toHaveBeenCalled();
  const search = f.element.querySelector<HTMLInputElement>('input')!;
  search.value = 'missing';
  search.dispatchEvent(new Event('input', { bubbles: true }));
  await nextTick();
  expect(f.element.textContent).toContain('No cubes found.');
  expect(f.element.querySelector('[aria-label="Cube cart"]')?.textContent).toContain('first');
});

test('uses keyed controls for keyboard draft permutations and keeps busy edits disabled without losing focus', async () => {
  const f = fixture();
  const button = f.cartButton(0);
  button.focus();
  button.dispatchEvent(
    new KeyboardEvent('keydown', {
      key: 'ArrowDown',
      altKey: true,
      bubbles: true,
      cancelable: true,
    }),
  );
  expect(f.actions.reorder).toHaveBeenCalledWith(['second', 'first']);
  f.view.update({ ...f.state, rows: [...f.state.rows].reverse(), pending: true });
  await nextTick();
  expect(f.cartButton(1)).toBe(button);
  expect(document.activeElement).toBe(button);
  f.libraryButton().click();
  f.element
    .querySelector<HTMLButtonElement>('[aria-label="Cube cart"] .sc-cube-stack-row__remove')!
    .click();
  expect(f.actions.add).not.toHaveBeenCalled();
  expect(f.actions.remove).not.toHaveBeenCalled();
});

test('a library Cube carried over the cart is previewed there and added where it is dropped', async () => {
  const f = fixture();
  const source = f.libraryButton().closest('li')!;
  source.dispatchEvent(pointer('pointerdown', 50, 23));
  document.dispatchEvent(pointer('pointermove', 60, 60));
  await nextTick();
  // Away from the cart the copy follows the pointer and the cart is unchanged.
  expect(f.element.querySelector('.sc-cube-stack__drag-overlay')?.textContent).toContain(
    'Library cube',
  );
  expect(f.cartLabels()).toEqual(['first', 'second']);
  document.dispatchEvent(pointer('pointermove', 400, 60));
  await nextTick();
  expect(f.cartLabels()).toEqual(['first', '<b>Library cube</b>', 'second']);
  expect(f.cartButton(1).closest('li')?.hasAttribute('data-dragging')).toBe(true);
  document.dispatchEvent(pointer('pointerup', 400, 60));
  await nextTick();
  expect(f.actions.add).toHaveBeenCalledWith('artifact', 'second');
  expect(f.actions.reorder).not.toHaveBeenCalled();
  expect(f.element.querySelector('.sc-cube-stack__drag-overlay')).toBeNull();
});

test('a library Cube released away from the cart is not added', async () => {
  const f = fixture();
  f.libraryButton()
    .closest('li')!
    .dispatchEvent(pointer('pointerdown', 50, 23));
  document.dispatchEvent(pointer('pointermove', 400, 60));
  document.dispatchEvent(pointer('pointermove', 60, 60));
  await nextTick();
  expect(f.cartLabels()).toEqual(['first', 'second']);
  document.dispatchEvent(pointer('pointerup', 60, 60));
  await nextTick();
  expect(f.actions.add).not.toHaveBeenCalled();
});

test('a lifted cart row previews its new place, and Escape puts it back without changing the draft', async () => {
  const f = fixture();
  const source = f.cartButton(0).closest('li')!;
  source.dispatchEvent(pointer('pointerdown', 400, 23));
  document.dispatchEvent(pointer('pointermove', 400, 80));
  await nextTick();
  expect(f.cartLabels()).toEqual(['second', 'first']);
  expect(f.cartButton(1).closest('li')?.hasAttribute('data-dragging')).toBe(true);
  const escape = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true });
  const reachedDialog = jest.fn();
  document.body.addEventListener('keydown', reachedDialog);
  document.body.dispatchEvent(escape);
  document.body.removeEventListener('keydown', reachedDialog);
  await nextTick();
  // Escape ends the drag alone; the dialog around the cart never hears it.
  expect(reachedDialog).not.toHaveBeenCalled();
  expect(f.cartLabels()).toEqual(['first', 'second']);
  expect(f.actions.reorder).not.toHaveBeenCalled();
  f.cartButton(0)
    .closest('li')!
    .dispatchEvent(pointer('pointerdown', 400, 23));
  document.dispatchEvent(pointer('pointermove', 400, 80));
  document.dispatchEvent(pointer('pointerup', 400, 80));
  await nextTick();
  expect(f.actions.reorder).toHaveBeenCalledWith(['second', 'first']);
});

test('switches appearance while retaining captured rows and search value', async () => {
  const f = fixture();
  const button = f.cartButton(0);
  const search = f.element.querySelector<HTMLInputElement>('input')!;
  search.value = 'literal';
  search.dispatchEvent(new Event('input', { bubbles: true }));
  f.view.update({ ...f.state, appearance: 'fluent' });
  await nextTick();
  expect(f.cartButton(0)).toBe(button);
  expect(f.element.querySelector<FinderInputControl>('fluent-text-input')?.value).toBe('literal');
  expect(f.element.querySelector('.sc-cube-stack')?.getAttribute('data-appearance')).toBe('fluent');
  f.view.update({ ...f.state, appearance: 'spectrum' });
  await nextTick();
  const spectrum = f.element.querySelector('sc-spectrum-search');
  if (!(spectrum instanceof Textfield))
    throw new TypeError('Expected the real Adobe search field.');
  spectrum.focus();
  await spectrum.updateComplete;
  await Promise.resolve();
  expect(spectrum.value).toBe('literal');
  expect(spectrum.shadowRoot?.querySelector('input')?.getAttribute('aria-label')).toBe(
    'Search cubes',
  );
  expect(document.activeElement).toBe(spectrum);
  expect(f.cartButton(0)).toBe(button);
  spectrum.value = 'missing';
  spectrum.dispatchEvent(new Event('input', { bubbles: true }));
  await nextTick();
  expect(f.element.textContent).toContain('No cubes found.');
  f.view.update({ ...f.state, appearance: 'spectrum', pending: true });
  await nextTick();
  expect(f.element.querySelector('sc-spectrum-search')).toBe(spectrum);
  expect(spectrum.value).toBe('missing');
});

/** Bind real Vue applications while mocking only externally owned draft commands and canvas drawing. */
function fixture() {
  jest.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null);
  const base = {
    secondary: 'v1',
    tooltip: 'literal',
    icon: createFallbackIconModel('CU'),
    targetModel: 'SDXL',
    selected: false,
    bypassed: false,
    severity: 'none' as const,
  };
  const library: CubeStackLibraryRow = {
    ...base,
    id: 'artifact',
    label: '<b>Library cube</b>',
    catalogKey: 'artifact',
    models: ['SDXL'],
    role: 'Start cubes',
    searchText: 'literal library cube',
  };
  const state: CubeStackCartViewState = {
    library: [library],
    rows: ['first', 'second'].map((id) => ({ ...base, id, label: id })),
    pending: false,
    appearance: 'comfy',
  };
  const actions = {
    add: jest.fn<CubeStackCartViewActions['add']>(),
    remove: jest.fn<CubeStackCartViewActions['remove']>(),
    reorder: jest.fn<CubeStackCartViewActions['reorder']>(),
  };
  const element = document.createElement('div');
  document.body.append(element);
  const view = mountCubeStackCart(element, state, actions);
  releases.push(view.dispose);
  // jsdom lays nothing out: the cart's list sits at the top of the page, right of the library.
  const cart = element.querySelector<HTMLElement>('.sc-cube-stack-cart')!;
  cart.setPointerCapture = jest.fn();
  cart.releasePointerCapture = jest.fn();
  cart.hasPointerCapture = jest.fn(() => true);
  const drop = element.querySelector<HTMLElement>('.sc-cube-stack-cart__drop')!;
  drop.getBoundingClientRect = () =>
    ({ left: 300, right: 524, top: 0, bottom: 400, width: 224, height: 400 }) as DOMRect;
  return {
    element,
    state,
    actions,
    view,
    libraryButton: () =>
      element.querySelector<HTMLButtonElement>(
        '[aria-label="Cube library"] .sc-cube-stack-row__activate',
      )!,
    cartLabels: () =>
      [...element.querySelectorAll('[aria-label="Cube cart"] .sc-cube-stack-row__label')].map(
        (label) => label.textContent,
      ),
    cartButton: (index: number) =>
      element.querySelectorAll<HTMLButtonElement>(
        '[aria-label="Cube cart"] .sc-cube-stack-row__activate',
      )[index]!,
  };
}

/** Build the pointer event jsdom has no constructor for. */
function pointer(type: string, x: number, y: number): PointerEvent {
  const event = new MouseEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: y });
  Object.defineProperty(event, 'pointerId', { value: 1 });
  return event as PointerEvent;
}
