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
/** Prove captured workflow gestures coexist with controls, host updates and frame retirement. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { WorkflowTabPointerDrag } from '../../../frontend/comfyui/ui/surface/frame/WorkflowTabPointerDrag.js';

interface MotionStub {
  cancel: ReturnType<typeof jest.fn>;
  onfinish: (() => void) | null;
  oncancel: (() => void) | null;
}
const disposals: Array<() => void> = [];
afterEach(() => {
  disposals.splice(0).forEach((dispose) => dispose());
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

/** Model only the absent browser geometry boundary, including current inline displacement. */
function rect(left: number, width: number): DOMRect {
  return {
    x: left,
    y: 0,
    left,
    top: 0,
    right: left + width,
    bottom: 30,
    width,
    height: 30,
    toJSON: () => ({}),
  };
}

/** Deliver primary pointer input without JSDOM's missing PointerEvent implementation. */
function pointer(target: EventTarget, type: string, x: number, buttons = 1, pointerId = 1): void {
  const event = new MouseEvent(type, {
    bubbles: true,
    cancelable: true,
    button: 0,
    buttons,
    clientX: x,
    clientY: 15,
  });
  Object.defineProperties(event, { pointerId: { value: pointerId }, isPrimary: { value: true } });
  target.dispatchEvent(event);
}

/** Mount a real event lifetime with only capture, animation and layout mocked at browser boundaries. */
function mount(deferred = false) {
  const viewport = document.createElement('div');
  const list = document.createElement('div');
  viewport.append(list);
  document.body.append(viewport);
  Object.defineProperties(viewport, {
    clientWidth: { value: 540 },
    scrollWidth: { value: 760 },
    setPointerCapture: { value: jest.fn() },
    hasPointerCapture: { value: () => true },
    releasePointerCapture: { value: jest.fn() },
  });
  jest.spyOn(viewport, 'getBoundingClientRect').mockImplementation(() => rect(0, 540));
  const motions: MotionStub[] = [];
  const animate = jest.fn((_frames: Keyframe[], _options: KeyframeAnimationOptions): MotionStub => {
    const animation: MotionStub = { cancel: jest.fn(), onfinish: null, oncancel: null };
    motions.push(animation);
    return animation;
  });
  let tabs: HTMLElement[] = ['a', 'b', 'c'].map((id) => {
    const tab = document.createElement('div');
    tab.id = id;
    list.append(tab);
    Object.defineProperty(tab, 'animate', { value: animate });
    jest
      .spyOn(tab, 'getBoundingClientRect')
      .mockImplementation(() =>
        rect(
          [...list.children].indexOf(tab) * 180 +
            Number(/translateX\(([-.\d]+)px\)/u.exec(tab.style.transform)?.[1] ?? 0),
          180,
        ),
      );
    return tab;
  });
  Object.defineProperty(list, 'animate', { value: animate });
  tabs[0]!.setAttribute('aria-selected', 'true');
  const restored = jest.fn(() =>
    list.style.setProperty(
      '--sc-selected-offset',
      `${String(tabs.indexOf(tabs.find((tab) => tab.getAttribute('aria-selected') === 'true')!) * 180)}px`,
    ),
  );
  const report = jest.fn();
  const accept = (next: HTMLElement[]): void => {
    const positions = drag.beforeUpdate(next.map((tab) => tab.id));
    tabs = next;
    list.replaceChildren(...tabs);
    drag.afterUpdate(tabs, positions);
  };
  const reorder = jest.fn((from: number, to: number) => {
    if (deferred) return;
    const next = [...tabs];
    next.splice(to, 0, next.splice(from, 1)[0]!);
    accept(next);
  });
  const drag = new WorkflowTabPointerDrag(viewport, list, reorder, report, restored);
  drag.afterUpdate(tabs, null);
  restored();
  disposals.push(() => drag.dispose());
  return { viewport, list, drag, reorder, report, animate, motions, restored, accept, tabs };
}

test('ordinary clicks and sub-threshold movement preserve native activation without reordering', () => {
  const f = mount();
  const click = jest.fn();
  f.tabs[0]!.addEventListener('click', click);
  pointer(f.tabs[0]!, 'pointerdown', 50);
  pointer(document, 'pointermove', 55);
  pointer(document, 'pointerup', 55, 0);
  f.tabs[0]!.dispatchEvent(new MouseEvent('click', { bubbles: true, detail: 1 }));
  expect(click).toHaveBeenCalledTimes(1);
  expect(f.reorder).not.toHaveBeenCalled();
  expect(f.animate).not.toHaveBeenCalled();
});

test.each(['pointercancel', 'lostpointercapture', 'blur', 'contextmenu', 'Escape', 'buttons-lost'])(
  '%s settles without committing and retires capture',
  (reason) => {
    const f = mount();
    pointer(f.tabs[0]!, 'pointerdown', 50);
    pointer(document, 'pointermove', 230);
    if (reason === 'Escape')
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    else if (reason === 'blur') window.dispatchEvent(new Event('blur'));
    else if (reason === 'contextmenu')
      f.viewport.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true }));
    else
      pointer(
        reason === 'lostpointercapture' ? f.viewport : document,
        reason === 'buttons-lost' ? 'pointermove' : reason,
        230,
        0,
      );
    pointer(document, 'pointerup', 230, 0);
    expect(f.reorder).not.toHaveBeenCalled();
    expect(f.viewport.hasAttribute('data-sc-tab-dragging')).toBe(false);
    expect(f.tabs.every((tab) => tab.style.transform === '')).toBe(true);
    expect(f.animate).toHaveBeenCalled();
    expect(f.viewport.releasePointerCapture).toHaveBeenCalledWith(1);
  },
);

test('close controls, add buttons and unrelated pointer cancellation never commandeer a tab gesture', () => {
  const f = mount();
  const close = document.createElement('button');
  close.className = 'sc-workflow-close';
  f.tabs[0]!.append(close);
  pointer(close, 'pointerdown', 50);
  pointer(document, 'pointermove', 230);
  expect(f.drag.active).toBe(false);
  pointer(f.viewport, 'pointerdown', 50);
  pointer(document, 'pointermove', 230);
  expect(f.drag.active).toBe(false);
  pointer(f.tabs[0]!, 'pointerdown', 50);
  pointer(document, 'pointermove', 230);
  pointer(document, 'pointercancel', 230, 0, 2);
  expect(f.drag.active).toBe(true);
  pointer(document, 'pointerup', 230, 0);
  expect(f.reorder).toHaveBeenCalledTimes(1);
});

test('metadata refresh preserves preview; accepted host order settles from painted positions once', () => {
  const f = mount(true);
  const click = jest.fn();
  f.tabs[0]!.addEventListener('click', click);
  pointer(f.tabs[0]!, 'pointerdown', 50);
  pointer(document, 'pointermove', 230);
  f.accept([...f.tabs]);
  expect(f.drag.active).toBe(true);
  pointer(document, 'pointerup', 230, 0);
  pointer(f.viewport, 'lostpointercapture', 230, 0);
  expect(f.drag.active).toBe(true);
  expect(f.reorder).toHaveBeenCalledWith(0, 1);
  f.accept([f.tabs[1]!, f.tabs[0]!, f.tabs[2]!]);
  expect(f.drag.active).toBe(false);
  expect([...f.list.children]).toEqual([f.tabs[1], f.tabs[0], f.tabs[2]]);
  f.tabs[0]!.dispatchEvent(new MouseEvent('click', { bubbles: true, detail: 1 }));
  expect(click).not.toHaveBeenCalled();
  f.tabs[0]!.dispatchEvent(new MouseEvent('click', { bubbles: true, detail: 0 }));
  expect(click).toHaveBeenCalledTimes(1);
});

test('external membership changes cancel the candidate and never commit a stale index', () => {
  const f = mount();
  pointer(f.tabs[0]!, 'pointerdown', 50);
  pointer(document, 'pointermove', 230);
  f.accept([f.tabs[0]!, f.tabs[2]!]);
  pointer(document, 'pointerup', 230, 0);
  expect(f.reorder).not.toHaveBeenCalled();
  expect(f.drag.active).toBe(false);
});

test('host failures report the original cause and restore accepted order', () => {
  const f = mount();
  const failure = new Error('host order rejected');
  f.reorder.mockImplementation(() => {
    throw failure;
  });
  pointer(f.tabs[0]!, 'pointerdown', 50);
  pointer(document, 'pointermove', 230);
  pointer(document, 'pointerup', 230, 0);
  expect(f.report).toHaveBeenCalledWith(failure);
  expect(f.drag.active).toBe(false);
  expect([...f.list.children]).toEqual(f.tabs);
});

test('preview cutouts and selected outline follow the held tab; cancel restores canonical shape', () => {
  const f = mount();
  pointer(f.tabs[0]!, 'pointerdown', 50);
  pointer(document, 'pointermove', 140);
  expect(f.tabs[0]!.style.getPropertyValue('--sc-tab-cutout-radius')).toBe('12.5px');
  expect(f.tabs[1]!.style.getPropertyValue('--sc-tab-cutout-radius')).toBe('12.5px');
  expect(f.tabs[0]!.style.paddingLeft).toBe('26px');
  expect(f.tabs[1]!.style.paddingLeft).toBe('26px');
  expect(f.list.style.getPropertyValue('--sc-selected-offset')).toBe('90px');
  f.viewport.dispatchEvent(new Event('contextmenu', { bubbles: true }));
  expect(f.tabs[0]!.style.getPropertyValue('--sc-tab-cutout-radius')).toBe('');
  expect(f.tabs[0]!.style.paddingLeft).toBe('');
  expect(f.list.style.getPropertyValue('--sc-selected-offset')).toBe('0px');
});

test('overflow edge scrolling reaches offscreen slots and unmount stops every resource', () => {
  jest.useFakeTimers();
  try {
    const f = mount();
    pointer(f.tabs[0]!, 'pointerdown', 50);
    pointer(document, 'pointermove', 535);
    jest.advanceTimersByTime(48);
    expect(f.viewport.scrollLeft).toBeGreaterThan(0);
    f.drag.dispose();
    const scroll = f.viewport.scrollLeft;
    jest.advanceTimersByTime(64);
    expect(f.viewport.scrollLeft).toBe(scroll);
    pointer(document, 'pointerup', 535, 0);
    expect(f.reorder).not.toHaveBeenCalled();
    expect(f.motions.every((animation) => animation.cancel.mock.calls.length > 0)).toBe(true);
  } finally {
    jest.useRealTimers();
  }
});

test('reduced motion retains drag and commit but omits settle animation', () => {
  const descriptor = Object.getOwnPropertyDescriptor(window, 'matchMedia');
  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    value: () => ({ matches: true }),
  });
  try {
    const f = mount();
    pointer(f.tabs[0]!, 'pointerdown', 50);
    pointer(document, 'pointermove', 230);
    pointer(document, 'pointerup', 250, 0);
    expect(f.reorder).toHaveBeenCalledWith(0, 1);
    expect(f.animate).not.toHaveBeenCalled();
  } finally {
    if (descriptor) Object.defineProperty(window, 'matchMedia', descriptor);
    else Reflect.deleteProperty(window, 'matchMedia');
  }
});

test('held painting occludes sibling text throughout settle and old cancellation cannot erase a new hold', () => {
  const f = mount();
  f.viewport.style.backgroundColor = 'rgb(32, 32, 32)';
  pointer(f.tabs[0]!, 'pointerdown', 50);
  pointer(document, 'pointermove', 250);
  expect(f.list.style.getPropertyValue('--sc-tab-drag-backdrop')).toBe('rgb(32, 32, 32)');
  pointer(document, 'pointerup', 250, 0);
  expect(f.tabs[0]!.hasAttribute('data-sc-held')).toBe(true);
  expect(f.tabs[0]!.style.transition).toBe('none');
  const old = [...f.motions];
  pointer(f.tabs[0]!, 'pointerdown', 250);
  pointer(document, 'pointermove', 50);
  old.forEach((animation) => animation.oncancel?.());
  expect(f.tabs[0]!.hasAttribute('data-sc-held')).toBe(true);
  f.drag.dispose();
  expect(f.tabs.every((tab) => !tab.hasAttribute('data-sc-held'))).toBe(true);
  expect(f.tabs.every((tab) => tab.style.transition === '')).toBe(true);
});

test('viewport scrolling preserves the held grab position and updates the release target', () => {
  const f = mount();
  pointer(f.tabs[0]!, 'pointerdown', 50);
  pointer(document, 'pointermove', 230);
  f.viewport.scrollLeft = 100;
  f.viewport.dispatchEvent(new Event('scroll'));
  expect(f.tabs[0]!.style.transform).toBe('translateX(280px)');
  pointer(document, 'pointerup', 230, 0);
  expect(f.reorder).toHaveBeenCalledWith(0, 2);
});

test('grabbing the leading or trailing edge yields identical displacement for the same travel', () => {
  for (const press of [5, 170]) {
    const f = mount();
    pointer(f.tabs[0]!, 'pointerdown', press);
    pointer(document, 'pointermove', press + 20);
    expect(f.tabs[1]!.style.transform).toBe('translateX(0px)');
    pointer(document, 'pointermove', press + 97);
    expect(f.tabs[1]!.style.transform).toBe('translateX(-180px)');
    pointer(document, 'pointermove', press + 89);
    expect(f.tabs[1]!.style.transform).toBe('translateX(-180px)');
    pointer(document, 'pointermove', press + 80);
    expect(f.tabs[1]!.style.transform).toBe('translateX(0px)');
    f.drag.dispose();
  }
});
