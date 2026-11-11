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
/** Prove the mounted dock lays its history, options and rail out around the route tabs. */
import { afterEach, describe, expect, jest, test } from '@jest/globals';
import { editState } from '../support/CanvasEditHostFixture.js';
import {
  clientRect,
  disposeEditing,
  flush,
  mountEditing,
  VIEWPORT,
} from '../support/CanvasEditOverlayFixture.js';

afterEach(() => {
  disposeEditing();
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

/** The route tabs at the canvas corner, in client pixels: overlay-local (8, 8), 140 by 40. */
const ROUTE_TABS = clientRect({
  left: VIEWPORT.left + 8,
  top: VIEWPORT.top + 8,
  width: 140,
  height: 40,
});

/** Size the top bar from its content (the open Brush options grow it) and the rail fixed. */
function sizeChrome(): void {
  const size = (element: HTMLElement): { width: number; height: number } => {
    if (element.classList.contains('sc-canvas-edit-rail-dock')) return { width: 40, height: 260 };
    if (!element.classList.contains('sc-canvas-edit-topbar')) return { width: 200, height: 36 };
    const options = element.querySelector('.sc-canvas-edit-options');
    if (!options) return { width: 64, height: 36 };
    return { width: 260, height: options.querySelector('input[type="range"]') ? 150 : 36 };
  };
  jest.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockImplementation(function (
    this: HTMLElement,
  ) {
    return size(this).width;
  });
  jest.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockImplementation(function (
    this: HTMLElement,
  ) {
    return size(this).height;
  });
}

function placement(root: HTMLElement, selector: string): string | undefined {
  return root.querySelector<HTMLElement>(selector)?.dataset.canvasPlacement;
}

describe('editing dock layout', () => {
  test('history follows the route tabs along the top row and the rail starts below them', async () => {
    const f = await mountEditing(editState(), () => [ROUTE_TABS]);
    sizeChrome();
    f.mounted.geometryChanged();
    await flush();
    expect(placement(f.root, '.sc-canvas-edit-topbar')).toBe('156,8');
    expect(placement(f.root, '.sc-canvas-edit-rail-dock')).toBe('8,56');
    expect(f.query('.sc-canvas-edit-topbar')?.style.transform).toBe('translate(156px, 8px)');
  });

  test('the open Brush options sit beside the route tabs instead of covering Output', async () => {
    const f = await mountEditing(editState({ tool: 'brush' }), () => [ROUTE_TABS]);
    sizeChrome();
    f.mounted.geometryChanged();
    await flush();
    const header = f.query('.sc-canvas-edit-options-header > button');
    if (!header) throw new Error('Missing Brush options header');
    header.click();
    await flush();
    expect(f.query('.sc-canvas-edit-options input[type="range"]')).not.toBeNull();
    f.mounted.geometryChanged();
    await flush();
    expect(placement(f.root, '.sc-canvas-edit-topbar')).toBe('156,8');
    expect(placement(f.root, '.sc-canvas-edit-rail-dock')).toBe('8,56');
  });

  test('the rail follows the history bar as the open options grow it', async () => {
    const observed = new Set<Element>();
    const resizes: (() => void)[] = [];
    class RecordingResizeObserver implements ResizeObserver {
      constructor(callback: ResizeObserverCallback) {
        resizes.push(() => callback([], this));
      }
      observe(target: Element): void {
        observed.add(target);
      }
      unobserve(target: Element): void {
        observed.delete(target);
      }
      disconnect(): void {
        observed.clear();
      }
    }
    Object.defineProperty(globalThis, 'ResizeObserver', {
      configurable: true,
      value: RecordingResizeObserver,
    });
    try {
      const f = await mountEditing(editState({ tool: 'brush' }));
      sizeChrome();
      f.mounted.geometryChanged();
      await flush();
      expect(placement(f.root, '.sc-canvas-edit-rail-dock')).toBe('8,52');
      expect([...observed].map((element) => element.className)).toEqual([
        'sc-canvas-edit-topbar',
        'sc-canvas-edit-rail-dock',
      ]);
      f.query('.sc-canvas-edit-options-header > button')?.click();
      await flush();
      expect(resizes).toHaveLength(1);
      for (const resize of resizes) resize();
      expect(placement(f.root, '.sc-canvas-edit-topbar')).toBe('8,8');
      expect(placement(f.root, '.sc-canvas-edit-rail-dock')).toBe('8,166');
      f.mounted.attach(null);
      await flush();
      expect(observed.size).toBe(0);
    } finally {
      Reflect.deleteProperty(globalThis, 'ResizeObserver');
    }
  });

  test('without route tabs the dock keeps the corner with the rail beneath the history', async () => {
    const f = await mountEditing(editState());
    sizeChrome();
    f.mounted.geometryChanged();
    await flush();
    expect(placement(f.root, '.sc-canvas-edit-topbar')).toBe('8,8');
    expect(placement(f.root, '.sc-canvas-edit-rail-dock')).toBe('8,52');
  });
});
