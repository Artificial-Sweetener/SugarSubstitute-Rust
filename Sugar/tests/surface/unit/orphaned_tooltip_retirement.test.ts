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
/** Prove a ComfyUI tooltip that has lost its element, or its pointer, does not stay on the page. */
import { afterEach, beforeEach, expect, jest, test } from '@jest/globals';
import { OrphanedTooltipRetirement } from '../../../frontend/comfyui/ui/surface/appearance/OrphanedTooltipRetirement.js';

let retirement: OrphanedTooltipRetirement;

beforeEach(() => {
  jest.useFakeTimers();
  retirement = new OrphanedTooltipRetirement(document);
});

afterEach(() => {
  retirement.dispose();
  jest.useRealTimers();
  document.body.replaceChildren();
});

/** A field row as PrimeVue's tooltip directive marks it. */
function row(label: string): HTMLElement {
  const element = document.createElement('div');
  element.setAttribute('data-pd-tooltip', 'true');
  element.textContent = label;
  document.body.append(element);
  return element;
}

/** Show `owner`'s tooltip the way the directive does: on the body, named on its element. */
function show(owner: HTMLElement, id: string): HTMLElement {
  const tooltip = document.createElement('div');
  tooltip.id = id;
  tooltip.className = 'p-tooltip p-component p-tooltip-left';
  tooltip.setAttribute('role', 'tooltip');
  document.body.append(tooltip);
  Reflect.set(owner, '$_ptooltipId', id);
  return tooltip;
}

function pointerOver(element: Element): void {
  element.dispatchEvent(new MouseEvent('pointermove', { bubbles: true }));
}

/** Let the page's mutation delivery reach the owner. */
async function delivered(): Promise<void> {
  await Promise.resolve();
}

test('a tooltip shown for a row that was rebuilt meanwhile is removed at once', async () => {
  const old = row('Enable');
  pointerOver(old);
  // The row is rebuilt while its tooltip is still waiting to be shown: the old element is gone
  // from the page when the tooltip arrives, and the new one names no tooltip.
  old.remove();
  const rebuilt = row('Enable');
  const tooltip = show(old, 'pv_id_289_tooltip');
  await delivered();
  expect(tooltip.isConnected).toBe(false);
  expect(rebuilt.isConnected).toBe(true);
});

test('a tooltip whose row is removed while it is shown goes with the next pointer movement', async () => {
  const owner = row('Enable');
  const elsewhere = row('Strength');
  pointerOver(owner);
  const tooltip = show(owner, 'pv_id_1_tooltip');
  await delivered();
  expect(tooltip.isConnected).toBe(true);
  owner.remove();
  pointerOver(elsewhere);
  expect(tooltip.isConnected).toBe(false);
});

test('a tooltip shown after the pointer left its row is removed once PrimeVue had its chance', async () => {
  const first = row('Enable');
  const second = row('Strength');
  pointerOver(first);
  pointerOver(second);
  // The directive's one shared timer shows the first row's tooltip although the pointer left it.
  const tooltip = show(first, 'pv_id_2_tooltip');
  await delivered();
  expect(tooltip.isConnected).toBe(true);
  jest.advanceTimersByTime(1000);
  expect(tooltip.isConnected).toBe(false);
});

test('a tooltip stays while the pointer is in its row or on the tooltip, and with keyboard focus', async () => {
  const owner = row('Enable');
  const elsewhere = row('Strength');
  pointerOver(owner);
  const tooltip = show(owner, 'pv_id_3_tooltip');
  await delivered();
  pointerOver(owner);
  jest.advanceTimersByTime(1000);
  expect(tooltip.isConnected).toBe(true);
  pointerOver(tooltip);
  jest.advanceTimersByTime(1000);
  expect(tooltip.isConnected).toBe(true);
  // The pointer leaves and comes back before PrimeVue's own removal would have run.
  pointerOver(elsewhere);
  pointerOver(owner);
  jest.advanceTimersByTime(1000);
  expect(tooltip.isConnected).toBe(true);
  owner.tabIndex = 0;
  owner.focus();
  pointerOver(elsewhere);
  jest.advanceTimersByTime(1000);
  expect(tooltip.isConnected).toBe(true);
});

test('a tooltip opened from the keyboard before any pointer movement is left alone', async () => {
  const owner = row('Enable');
  const tooltip = show(owner, 'pv_id_4_tooltip');
  await delivered();
  jest.advanceTimersByTime(1000);
  expect(tooltip.isConnected).toBe(true);
});

test('a tooltip without an identity, such as one of SugarCubes own, is never touched', async () => {
  const elsewhere = row('Strength');
  const own = document.createElement('div');
  own.className = 'p-tooltip p-component p-tooltip-top';
  document.body.append(own);
  await delivered();
  pointerOver(elsewhere);
  jest.advanceTimersByTime(1000);
  expect(own.isConnected).toBe(true);
});
