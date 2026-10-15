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
/** Prove live pointer reorder previews, canonical release and cancellation through official tabs. */
import { afterAll, afterEach, beforeAll, expect, jest, test } from '@jest/globals';
import { Updates } from '@microsoft/fast-element';
import { WorkflowTabsView } from '../../../frontend/comfyui/ui/surface/frame/WorkflowTabsView.js';
import { substituteFrameCss } from '../../../frontend/comfyui/ui/surface/frame/SubstituteFrameStyles.js';
import { workflowSessionId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';

const internals = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'attachInternals');
beforeAll(() => {
  // JSDOM omits the shadow-root internals required by Microsoft's actual focus group.
  Object.defineProperty(HTMLElement.prototype, 'attachInternals', {
    configurable: true,
    value: function (this: HTMLElement) {
      const host = this;
      return {
        role: null,
        states: new Set<string>(),
        get shadowRoot() {
          return host.shadowRoot;
        },
      };
    },
  });
});
afterAll(() => {
  if (internals) Object.defineProperty(HTMLElement.prototype, 'attachInternals', internals);
  else Reflect.deleteProperty(HTMLElement.prototype, 'attachInternals');
});
afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test('selected-corner pseudo-elements inherit the host offset that Chromium animates', () => {
  // JSDOM cannot compute shadow pseudo-elements; enforce the native CSS property contract here.
  const registration = /@property --sc-selected-offset\s*\{([^}]+)\}/u.exec(
    substituteFrameCss,
  )?.[1];
  expect(registration).toMatch(/inherits\s*:\s*true\s*;/u);
});

/** Deliver the browser pointer boundary without relying on JSDOM's missing PointerEvent constructor. */
function pointer(
  target: Element,
  type: string,
  x: number,
  options: { buttons?: number; pointerId?: number } = {},
): void {
  const event = new MouseEvent(type, {
    bubbles: true,
    cancelable: true,
    button: 0,
    buttons: options.buttons ?? 1,
    clientX: x,
    clientY: 15,
  });
  Object.defineProperties(event, {
    pointerId: { value: options.pointerId ?? 1 },
    isPrimary: { value: true },
    pointerType: { value: 'mouse' },
  });
  target.dispatchEvent(event);
}

test('pointer movement animates a transient permutation and commits once on release', async () => {
  const viewport = document.createElement('div');
  const capture = jest.fn();
  Object.defineProperties(viewport, {
    clientWidth: { value: 540 },
    setPointerCapture: { value: capture },
    hasPointerCapture: { value: () => true },
    releasePointerCapture: { value: jest.fn() },
  });
  jest.spyOn(viewport, 'getBoundingClientRect').mockImplementation(() => ({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: 540,
    bottom: 30,
    width: 540,
    height: 30,
    toJSON: () => ({}),
  }));
  let tabs = ['a', 'b', 'c'].map((key, index) => ({
    id: workflowSessionId(key),
    title: key,
    selected: index === 0,
    modified: false,
  }));
  const reorder = jest.fn((from: number, to: number) => {
    const next = [...tabs];
    const moved = next.splice(from, 1)[0]!;
    next.splice(to, 0, moved);
    tabs = next;
    view.update(tabs, () => false);
  });
  const view = new WorkflowTabsView(
    document,
    { activate: jest.fn(), close: jest.fn(), reorder, create: jest.fn(), report: jest.fn() },
    viewport,
  );
  viewport.append(view.element);
  document.body.append(viewport);
  view.update(tabs, () => false);
  await Updates.next();
  const ids = tabs.map((tab) => `sc-workflow-${tab.id}`);
  const a = document.getElementById(ids[0]!)!;
  const b = document.getElementById(ids[1]!)!;
  pointer(a, 'pointerdown', 50);
  pointer(viewport, 'pointermove', 230);
  expect(viewport.hasAttribute('data-sc-tab-dragging')).toBe(true);
  expect(a.style.transform).toBe('translateX(180px)');
  expect(b.style.transform).toBe('translateX(-180px)');
  expect(reorder).not.toHaveBeenCalled();
  expect([...view.element.children].map((e) => e.id)).toEqual(ids);
  pointer(viewport, 'pointerup', 230, { buttons: 0 });
  expect(reorder).toHaveBeenCalledTimes(1);
  expect(reorder).toHaveBeenCalledWith(0, 1);
  expect([...view.element.children].map((e) => e.id)).toEqual([ids[1], ids[0], ids[2]]);
  expect(capture).toHaveBeenCalledWith(1);
  view.dispose();
});
