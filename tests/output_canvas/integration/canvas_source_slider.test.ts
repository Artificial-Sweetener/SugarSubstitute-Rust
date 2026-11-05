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
/** Prove the sliding source selection and the navigation bar's arrow cursor and glass surface. */

import { jest } from '@jest/globals';
import { nextTick } from 'vue';
import { mountCanvasOverlay } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasOverlayRuntime.js';
import type { CanvasViewSnapshot } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import { CanvasPortFixture, canvasSnapshot } from '../support/CanvasPortFixture.js';

afterEach(() => {
  jest.restoreAllMocks();
  document.body.replaceChildren();
  document.getElementById('sugarcubes-output-overlay-styles')?.remove();
});

/** Lay tabs out side by side, 100 px each, as a browser would. */
function layOutTabs(): void {
  const position = (element: HTMLElement): number =>
    element.getAttribute('role') === 'tab' && element.parentElement
      ? [...element.parentElement.querySelectorAll('[role="tab"]')].indexOf(element)
      : -1;
  jest.spyOn(HTMLElement.prototype, 'offsetLeft', 'get').mockImplementation(function (
    this: HTMLElement,
  ) {
    return Math.max(0, position(this)) * 100;
  });
  jest.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockImplementation(function (
    this: HTMLElement,
  ) {
    return position(this) >= 0 ? 100 : 0;
  });
}

/** Read the style rule declared for an exact selector in the overlay stylesheet. */
function rule(selector: string): CSSStyleDeclaration | null {
  const sheet = document.getElementById('sugarcubes-output-overlay-styles');
  if (!(sheet instanceof HTMLStyleElement) || !sheet.sheet) return null;
  for (const entry of Array.from(sheet.sheet.cssRules))
    if (
      entry instanceof CSSStyleRule &&
      entry.selectorText.replaceAll('"', '') === selector.replaceAll('"', '')
    )
      return entry.style;
  return null;
}

test('one selection indicator covers the selected tab and slides to the next selection', async () => {
  layOutTabs();
  const initial = canvasSnapshot('slider');
  const port = new CanvasPortFixture(initial);
  const root = document.createElement('section');
  document.body.append(root);
  const mounted = mountCanvasOverlay(root, port, jest.fn());
  const indicators = root.querySelectorAll<HTMLElement>('.sc-output-slider-indicator');
  expect(indicators).toHaveLength(1);
  const indicator = indicators[0]!;
  expect(indicator.hidden).toBe(false);
  expect(indicator.style.transform).toBe('translateX(0px)');
  expect(indicator.style.width).toBe('100px');
  expect(indicator.dataset.placed).toBe('true');
  const next: CanvasViewSnapshot = {
    ...initial,
    revision: 2,
    view: {
      ...initial.view,
      revision: 2,
      selection: { sourceId: initial.sources[1]!.id, batchIndex: 2 },
    },
  };
  port.publish(next);
  await nextTick();
  expect(root.querySelectorAll('.sc-output-slider-indicator')[0]).toBe(indicator);
  expect(indicator.style.transform).toBe('translateX(100px)');
  expect(
    rule('.sc-output-slider-indicator[data-placed]')?.getPropertyValue('transition'),
  ).toContain('transform 200ms');
  expect(rule('.sc-output-tabs button[aria-selected="true"]')?.getPropertyValue('background')).toBe(
    'transparent',
  );
  mounted.dispose();
});

test('navigation pickers are chevron combo boxes and show the arrow cursor on a glass bar', () => {
  const root = document.createElement('section');
  document.body.append(root);
  const mounted = mountCanvasOverlay(
    root,
    new CanvasPortFixture(canvasSnapshot('glass')),
    jest.fn(),
  );
  expect(rule('.sc-output-tabs button,.sc-output-overlay .sc-output-segment')?.cursor).toBe(
    'default',
  );
  const bar = rule('.sc-output-tabstrip');
  expect(bar?.getPropertyValue('backdrop-filter')).toBe('blur(var(--p-toast-blur,10px))');
  expect(bar?.getPropertyValue('background')).toContain('transparent');
  expect(rule('.sc-output-tabs.sc-output-tabs-measure')?.position).toBe('absolute');
  const tab = root.querySelector('[role="tab"]')!;
  const batch = root.querySelector('button[aria-label="Output image"]')!;
  expect(getComputedStyle(tab).cursor).toBe('default');
  expect(getComputedStyle(batch).cursor).toBe('default');
  expect(batch.querySelector('.sc-output-chevron')?.getAttribute('aria-hidden')).toBe('true');
  expect(rule('.sc-output-overlay .sc-output-segment')?.getPropertyValue('border')).toContain(
    '1px solid',
  );
  mounted.dispose();
});
