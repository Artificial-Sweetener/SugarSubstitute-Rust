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
import { afterEach, expect, jest, test } from '@jest/globals';
import { nextTick } from 'vue';
import { createFallbackIconModel } from '../../../frontend/comfyui/ui/core/CubeFallbackIconRenderer.js';
import { mountCubeStackCart } from '../../../frontend/comfyui/ui/picker/vue/CubeStackCartRuntime.js';
import type {
  CubeStackCartViewActions,
  CubeStackCartViewState,
  CubeStackLibraryRow,
} from '../../../frontend/comfyui/ui/picker/CubeStackCartView.js';

const releases: (() => void)[] = [];
afterEach(() => {
  for (const release of releases.splice(0)) release();
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test.each(['fluent', 'comfy'] as const)(
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

test('dragging a captured library artifact stages it before an exact cart reference', async () => {
  const f = fixture();
  f.libraryButton().closest('li')!.dispatchEvent(drag('dragstart'));
  const target = f.cartButton(0).closest('li')!;
  target.dispatchEvent(drag('dragover'));
  target.dispatchEvent(drag('drop'));
  await nextTick();
  expect(f.actions.add).toHaveBeenCalledWith('artifact', 'first');
  expect(f.actions.reorder).not.toHaveBeenCalled();
});

test('previews cart ordering locally and discards a canceled drag without changing the draft', async () => {
  const f = fixture();
  const source = f.cartButton(0).closest('li')!;
  source.dispatchEvent(drag('dragstart'));
  f.cartButton(1).closest('li')!.dispatchEvent(drag('dragover'));
  await nextTick();
  expect(f.cartButton(0).textContent).toContain('second');
  source.dispatchEvent(drag('dragend'));
  await nextTick();
  expect(f.cartButton(0).textContent).toContain('first');
  expect(f.actions.reorder).not.toHaveBeenCalled();
  f.cartButton(0).closest('li')!.dispatchEvent(drag('drop'));
  expect(f.actions.reorder).not.toHaveBeenCalled();
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
  expect(f.element.querySelector('fluent-text-input')?.getAttribute('value')).toBe('literal');
  expect(f.element.querySelector('.sc-cube-stack')?.getAttribute('data-appearance')).toBe('fluent');
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
  return {
    element,
    state,
    actions,
    view,
    libraryButton: () =>
      element.querySelector<HTMLButtonElement>(
        '[aria-label="Cube library"] .sc-cube-stack-row__activate',
      )!,
    cartButton: (index: number) =>
      element.querySelectorAll<HTMLButtonElement>(
        '[aria-label="Cube cart"] .sc-cube-stack-row__activate',
      )[index]!,
  };
}

/** Provide jsdom's missing native drag fields without treating foreign transfer text as session authority. */
function drag(type: string): Event {
  const event = new Event(type, { bubbles: true, cancelable: true });
  Object.defineProperties(event, {
    dataTransfer: { value: { effectAllowed: 'none', dropEffect: 'none', setData: jest.fn() } },
    clientY: { value: 100 },
  });
  return event;
}
