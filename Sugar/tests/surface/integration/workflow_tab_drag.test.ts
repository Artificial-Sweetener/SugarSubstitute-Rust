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
import { WorkflowTabPointerDrag } from '../../../frontend/comfyui/ui/surface/frame/WorkflowTabPointerDrag.js';
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

test.each([true, false])(
  'pointer movement retains one reorder with orb visibility %s',
  async (orbVisible) => {
    const viewport = document.createElement('div');
    Object.defineProperties(viewport, {
      clientWidth: { value: 540 },
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
      {
        activate: jest.fn(),
        close: jest.fn(),
        menu: async () => [],
        reorder,
        create: jest.fn(),
        reopen: jest.fn(),
        reopenAvailable: () => false,
        report: jest.fn(),
      },
      viewport,
    );
    view.setOrbVisible(orbVisible);
    viewport.append(view.element);
    document.body.append(viewport);
    view.update(tabs, () => false);
    view.resize(tabs.map(() => 180));
    await Updates.next();
    const ids = tabs.map((tab) => `sc-workflow-${tab.id}`);
    const a = document.getElementById(ids[0]!)!;
    const b = document.getElementById(ids[1]!)!;
    const capture = captureBoundary([viewport, a, b]);
    pointer(a, 'pointerdown', 50);
    pointer(viewport, 'pointermove', 230);
    expect(viewport.hasAttribute('data-sc-tab-dragging')).toBe(true);
    expect(a.style.transform).toBe('translateX(180px)');
    if (!orbVisible) {
      expect(view.element.querySelector('[data-orb-adjacent]')).toBeNull();
      expect(a.style.getPropertyValue('--sc-tab-cutout-radius')).toBe('0px');
      expect(b.style.getPropertyValue('--sc-tab-cutout-radius')).toBe('0px');
      expect(a.style.paddingLeft).toBe('20px');
    }
    expect(b.style.transform).toBe('translateX(-180px)');
    expect(reorder).not.toHaveBeenCalled();
    expect([...view.element.children].map((e) => e.id)).toEqual(ids);
    pointer(viewport, 'pointerup', 230, { buttons: 0 });
    expect(reorder).toHaveBeenCalledTimes(1);
    expect(reorder).toHaveBeenCalledWith(0, 1);
    expect([...view.element.children].map((e) => e.id)).toEqual([ids[1], ids[0], ids[2]]);
    expect(capture.record).toHaveBeenCalledWith(a, 1);
    view.dispose();
  },
);

test('stationary captured presses retain the native control as the browser click target', () => {
  const viewport = document.createElement('div');
  const list = document.createElement('div');
  const activate = jest.fn();
  const reorder = jest.fn();
  const tabs = ['a', 'b'].map((id) => {
    const tab = document.createElement('button');
    tab.id = id;
    tab.addEventListener('click', () => activate(id));
    return tab;
  });
  list.append(...tabs);
  viewport.append(list);
  document.body.append(viewport);
  const drag = new WorkflowTabPointerDrag(viewport, list, reorder, jest.fn(), jest.fn(), () =>
    tabs.map(() => 180),
  );
  drag.afterUpdate(tabs, null);
  const pressed = tabs[1]!;
  const capture = captureBoundary([viewport, ...tabs]);
  try {
    pointer(pressed, 'pointerdown', 230);
    // Browsers determine the compatibility click target from pointerup's capture target.
    const releaseTarget = capture.owner ?? pressed;
    pointer(releaseTarget, 'pointerup', 230, { buttons: 0 });
    releaseTarget.dispatchEvent(new MouseEvent('click', { bubbles: true, detail: 1 }));
    expect(activate).toHaveBeenCalledTimes(1);
    expect(activate).toHaveBeenCalledWith('b');
    expect(reorder).not.toHaveBeenCalled();
    expect(capture.owner).toBeNull();
  } finally {
    drag.dispose();
  }
});

test('a middle click closes its tab once, even when the host omits auxclick, and never activates it', async () => {
  const viewport = document.createElement('div');
  document.body.append(viewport);
  const close = jest.fn();
  const activate = jest.fn();
  const id = workflowSessionId('middle-close');
  const other = workflowSessionId('middle-other');
  const view = new WorkflowTabsView(
    document,
    {
      activate,
      close,
      menu: async () => [],
      reorder: jest.fn(),
      create: jest.fn(),
      reopen: jest.fn(),
      reopenAvailable: () => false,
      report: jest.fn(),
    },
    viewport,
  );
  viewport.append(view.element);
  view.update(
    [
      { id: other, title: 'Shown', selected: true, modified: false },
      { id, title: 'Close me', selected: false, modified: false },
    ],
    () => false,
  );
  await Updates.next();
  const tab = document.getElementById(`sc-workflow-${id}`)!;
  const shown = document.getElementById(`sc-workflow-${other}`)!;
  const mouse = (target: Element, type: string, button: number): MouseEvent => {
    const event = new MouseEvent(type, { button, bubbles: true, cancelable: true });
    target.dispatchEvent(event);
    return event;
  };
  try {
    mouse(tab, 'mouseup', 0);
    mouse(tab, 'mouseup', 2);
    expect(close).not.toHaveBeenCalled();
    // A release with no press on this tab ends a gesture begun elsewhere, as a middle-button
    // pan of the canvas released over the tab strip does.
    mouse(tab, 'mouseup', 1);
    expect(close).not.toHaveBeenCalled();
    // A press on one tab released on another closes neither.
    mouse(shown, 'mousedown', 1);
    mouse(tab, 'mouseup', 1);
    expect(close).not.toHaveBeenCalled();
    // The press must not focus the tab: Fluent would select it and load its workflow.
    expect(mouse(tab, 'mousedown', 1).defaultPrevented).toBe(true);
    mouse(tab, 'mouseup', 1);
    expect(close).toHaveBeenCalledTimes(1);
    expect(close).toHaveBeenCalledWith(id);
    mouse(tab, 'auxclick', 1);
    mouse(tab, 'mouseup', 1);
    expect(close).toHaveBeenCalledTimes(1);
    // A browser that honours the cancelled pointer press delivers no mouse events for it: the
    // pointer release alone closes the tab, once, even if a mouse release follows.
    expect(mouse(tab, 'pointerdown', 1).defaultPrevented).toBe(true);
    mouse(tab, 'pointerup', 1);
    mouse(tab, 'mouseup', 1);
    expect(close).toHaveBeenCalledTimes(2);
    expect(activate).not.toHaveBeenCalled();
  } finally {
    view.dispose();
  }
});

/** Model the browser's exclusive capture owner, which JSDOM does not implement. */
function captureBoundary(elements: readonly HTMLElement[]) {
  let owner: HTMLElement | null = null;
  const record = jest.fn<(element: HTMLElement, pointerId: number) => void>();
  for (const element of elements)
    Object.defineProperties(element, {
      setPointerCapture: {
        value: (id: number) => {
          owner = element;
          record(element, id);
        },
      },
      hasPointerCapture: { value: () => owner === element },
      releasePointerCapture: {
        value: () => {
          owner = null;
        },
      },
    });
  return {
    record,
    get owner() {
      return owner;
    },
  };
}
