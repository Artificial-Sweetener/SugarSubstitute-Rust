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
/** Verify Cube faces reposition Comfy's real Nodes 2.0 boundary slots. */

import { jest } from '@jest/globals';

import { ComfyVueCubeBoundaryHost } from '../../frontend/comfyui/ui/surface/ComfyVueCubeBoundaryHost.js';

describe('ComfyVueCubeBoundaryHost', () => {
  test('places outputs across from preview titles while retaining evenly spaced inputs', () => {
    const body = document.createElement('div');
    setRect(body, { top: 100, left: 20, width: 250, height: 300 });
    Object.defineProperty(body, 'offsetWidth', { configurable: true, value: 500 });
    const row = document.createElement('div');
    setRect(row, { top: 80, left: 20, width: 250, height: 320 });
    const firstInput = slot('input');
    const secondInput = slot('input');
    const output = slot('output');
    row.append(firstInput, secondInput, output);
    const previewTitle = document.createElement('header');
    previewTitle.dataset.cubePreviewOutputTitle = '';
    setRect(previewTitle, { top: 150, left: 200, width: 60, height: 20 });
    body.append(row, previewTitle);

    const host = new ComfyVueCubeBoundaryHost(body);

    expect(row.dataset.sugarcubeBoundaryRow).toBe('');
    expect(firstInput.dataset.sugarcubeBoundaryDirection).toBe('input');
    expect(firstInput.dataset.sugarcubeBoundaryIndex).toBe('0');
    expect(firstInput.style.getPropertyValue('--sugarcube-boundary-position')).toBe(
      '33.333333333333336%',
    );
    expect(secondInput.dataset.sugarcubeBoundaryDirection).toBe('input');
    expect(secondInput.dataset.sugarcubeBoundaryIndex).toBe('1');
    expect(secondInput.style.getPropertyValue('--sugarcube-boundary-position')).toBe(
      '66.66666666666667%',
    );
    expect(output.dataset.sugarcubeBoundaryDirection).toBe('output');
    expect(output.dataset.sugarcubeBoundaryIndex).toBe('0');
    expect(output.style.getPropertyValue('--sugarcube-boundary-position')).toBe('160px');

    host.dispose();
  });

  test('restores host presentation and reconciles slots replaced by Vue', () => {
    const body = document.createElement('div');
    const firstRow = document.createElement('div');
    const firstSlot = slot('input');
    firstSlot.setAttribute('style', 'color: red');
    firstSlot.dataset.sugarcubeBoundaryDirection = 'preserved';
    firstRow.append(firstSlot);
    body.append(firstRow);
    const host = new ComfyVueCubeBoundaryHost(body);
    const secondRow = document.createElement('div');
    const secondSlot = slot('output');
    secondRow.append(secondSlot);

    firstRow.replaceWith(secondRow);
    host.reconcile();

    expect(secondRow.dataset.sugarcubeBoundaryRow).toBe('');
    expect(secondSlot.dataset.sugarcubeBoundaryDirection).toBe('output');
    host.dispose();
    expect(firstRow.dataset.sugarcubeBoundaryRow).toBeUndefined();
    expect(firstSlot.getAttribute('style')).toBe('color: red');
    expect(firstSlot.dataset.sugarcubeBoundaryDirection).toBe('preserved');
    expect(secondRow.dataset.sugarcubeBoundaryRow).toBeUndefined();
    expect(secondSlot.dataset.sugarcubeBoundaryDirection).toBeUndefined();
    expect(secondSlot.getAttribute('style')).toBeNull();
  });

  test('requests native slot remeasurement only when boundary presentation changes', () => {
    const body = document.createElement('div');
    const row = document.createElement('div');
    row.append(slot('input'), slot('output'));
    body.append(row);
    const requestSlotLayoutSync = jest.fn();

    const host = new ComfyVueCubeBoundaryHost(body, requestSlotLayoutSync);
    host.reconcile();
    expect(requestSlotLayoutSync).toHaveBeenCalledTimes(1);

    const replacement = document.createElement('div');
    replacement.append(slot('input'));
    row.replaceWith(replacement);
    host.reconcile();
    host.reconcile();

    expect(requestSlotLayoutSync).toHaveBeenCalledTimes(2);
  });
});

/** Build one real native slot-shaped element for a direction. */
function slot(direction: 'input' | 'output'): HTMLDivElement {
  const element = document.createElement('div');
  element.className = `lg-slot lg-slot--${direction}`;
  return element;
}

/** Give one JSDOM element finite viewport geometry. */
function setRect(
  element: HTMLElement,
  rect: { top: number; left: number; width: number; height: number },
): void {
  element.getBoundingClientRect = () =>
    ({
      x: rect.left,
      y: rect.top,
      top: rect.top,
      left: rect.left,
      width: rect.width,
      height: rect.height,
      right: rect.left + rect.width,
      bottom: rect.top + rect.height,
      toJSON: () => ({}),
    }) as DOMRect;
}
