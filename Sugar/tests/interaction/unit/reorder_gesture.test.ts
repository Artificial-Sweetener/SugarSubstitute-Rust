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
/** Prove the shared lifted-card gesture: when it lifts, what it previews, and how it ends. */
import { afterEach, expect, jest, test } from '@jest/globals';
import {
  ReorderGesture,
  type ReorderAdmission,
  type ReorderDropTarget,
} from '../../../frontend/comfyui/ui/interaction/reorder/ReorderGesture.js';
import { strideProjection } from '../../../frontend/comfyui/ui/interaction/reorder/ReorderSlots.js';

afterEach(() => document.body.replaceChildren());

function pointer(type: string, y: number, extra: PointerEventInit = {}): PointerEvent {
  const event = new MouseEvent(type, { bubbles: true, cancelable: true, clientX: 50, clientY: y });
  Object.defineProperty(event, 'pointerId', { value: extra.pointerId ?? 1 });
  if (extra.button !== undefined) Object.defineProperty(event, 'button', { value: extra.button });
  return event as PointerEvent;
}

/** A list of three 46 px rows a 50 px stride apart, starting at the top of the page. */
function list(
  admission: () => ReorderAdmission = () => 'ready',
  targets: ReorderDropTarget[] = [],
) {
  const holder = document.createElement('div');
  holder.setPointerCapture = jest.fn();
  holder.releasePointerCapture = jest.fn();
  holder.hasPointerCapture = jest.fn(() => true);
  const item = document.createElement('div');
  item.getBoundingClientRect = () => ({ left: 0, top: 0, width: 200, height: 46 }) as DOMRect;
  holder.append(item);
  document.body.append(holder);
  const calls = {
    lifted: jest.fn(),
    moved: jest.fn(),
    preview: jest.fn(),
    commit: jest.fn(),
    cancelled: jest.fn(),
  };
  const gesture = new ReorderGesture({
    document,
    threshold: 6,
    captureElement: () => holder,
    admission,
    keys: () => ['a', 'b', 'c'],
    project: (card, current) =>
      strideProjection({
        current,
        movable: () => true,
        key: card.key,
        center: card.y + 23,
        listStart: 0,
        stride: 50,
        size: 46,
      }),
    accepts: (_x, y) => y >= 0 && y <= 150,
    targets,
    ...calls,
  });
  const press = (y = 23, extra: PointerEventInit = {}): boolean =>
    gesture.press(pointer('pointerdown', y, extra), 'a', item);
  const move = (y: number): void => void document.dispatchEvent(pointer('pointermove', y));
  const release = (y: number): void => void document.dispatchEvent(pointer('pointerup', y));
  return { gesture, holder, item, calls, press, move, release };
}

test('a press lifts its card only once it has travelled, then previews and commits the new order', () => {
  const f = list();
  expect(f.press()).toBe(true);
  f.move(26);
  // Three pixels is a click, not a drag: nothing is captured or lifted.
  expect(f.gesture.card).toBeNull();
  expect(f.holder.setPointerCapture).not.toHaveBeenCalled();
  f.move(90);
  expect(f.holder.setPointerCapture).toHaveBeenCalledWith(1);
  expect(f.calls.lifted).toHaveBeenCalledWith('a');
  expect(f.gesture.card).toMatchObject({ key: 'a', x: 0, y: 67, width: 200, grabY: 23 });
  expect(f.calls.preview).toHaveBeenLastCalledWith(['b', 'a', 'c']);
  // Resting on the crossed boundary does not flip the order back.
  f.move(86);
  expect(f.calls.preview).toHaveBeenCalledTimes(1);
  f.release(90);
  expect(f.calls.commit).toHaveBeenCalledWith('a', ['b', 'a', 'c']);
  expect(f.calls.cancelled).not.toHaveBeenCalled();
  expect(f.gesture.card).toBeNull();
  expect(f.holder.releasePointerCapture).toHaveBeenCalledWith(1);
});

test('only the pressing pointer and the left button drive a drag', () => {
  const f = list();
  expect(f.press(23, { button: 2 })).toBe(false);
  expect(f.press()).toBe(true);
  expect(f.press()).toBe(false);
  document.dispatchEvent(pointer('pointermove', 120, { pointerId: 9 }));
  expect(f.gesture.card).toBeNull();
  document.dispatchEvent(pointer('pointerup', 120, { pointerId: 9 }));
  expect(f.gesture.pressed).toBe(true);
});

test.each(['escape', 'blur', 'cancel', 'outside', 'unchanged'])(
  '%s abandons a lifted card and restores the accepted order',
  (how) => {
    const f = list();
    f.press();
    f.move(how === 'unchanged' ? 40 : 90);
    expect(f.gesture.card).not.toBeNull();
    if (how === 'escape')
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    else if (how === 'blur') window.dispatchEvent(new Event('blur'));
    else if (how === 'cancel') f.gesture.cancel();
    else f.release(how === 'outside' ? 400 : 40);
    expect(f.calls.commit).not.toHaveBeenCalled();
    expect(f.calls.cancelled).toHaveBeenLastCalledWith(true);
    expect(f.calls.preview).toHaveBeenLastCalledWith(null);
    expect(f.calls.moved).toHaveBeenLastCalledWith(null);
    expect(f.gesture.card).toBeNull();
    // Nothing is left listening: later movement does nothing.
    f.move(140);
    expect(f.gesture.card).toBeNull();
  },
);

test('a press made while its caller is busy waits, and lifts where the pointer now is', () => {
  let admission: ReorderAdmission = 'defer';
  const f = list(() => admission);
  expect(f.press()).toBe(true);
  f.move(120);
  expect(f.gesture.card).toBeNull();
  admission = 'ready';
  f.gesture.resume();
  expect(f.gesture.card).toMatchObject({ y: 97 });
  // Its centre, at 120, is past the second row's and short of the third's.
  expect(f.calls.preview).toHaveBeenLastCalledWith(['b', 'a', 'c']);
  // A card that may no longer move when its caller is free is let go.
  const g = list(() => admission);
  admission = 'defer';
  g.press();
  g.move(120);
  admission = 'refuse';
  g.gesture.resume();
  expect(g.gesture.pressed).toBe(false);
  expect(g.calls.cancelled).toHaveBeenLastCalledWith(false);
});

test('the click a browser fires after a drag is swallowed, and a keyboard activation is not', () => {
  const f = list();
  f.press();
  f.move(90);
  f.release(90);
  const tail = new MouseEvent('click', { bubbles: true, cancelable: true, detail: 1 });
  f.gesture.click(tail);
  expect(tail.defaultPrevented).toBe(true);
  const next = new MouseEvent('click', { bubbles: true, cancelable: true, detail: 1 });
  f.gesture.click(next);
  expect(next.defaultPrevented).toBe(false);
  f.press();
  f.move(90);
  f.release(90);
  const keyboard = new MouseEvent('click', { bubbles: true, cancelable: true, detail: 0 });
  f.gesture.click(keyboard);
  expect(keyboard.defaultPrevented).toBe(false);
});

test('a card carried over another place is previewed and dropped there, not in its own list', () => {
  const target: ReorderDropTarget = {
    hit: (_x, y) => (y > 300 ? { index: 2 } : null),
    show: jest.fn(),
    accept: jest.fn(),
  };
  const f = list(() => 'ready', [target]);
  f.press();
  f.move(90);
  expect(f.calls.preview).toHaveBeenLastCalledWith(['b', 'a', 'c']);
  f.move(320);
  // Over the other place its own list shows the accepted order again.
  expect(f.calls.preview).toHaveBeenLastCalledWith(['a', 'b', 'c']);
  expect(target.show).toHaveBeenLastCalledWith('a', { index: 2 });
  f.move(90);
  expect(target.show).toHaveBeenLastCalledWith('a', null);
  f.move(320);
  f.release(320);
  expect(target.accept).toHaveBeenCalledWith('a', { index: 2 });
  expect(target.show).toHaveBeenLastCalledWith('a', null);
  expect(f.calls.commit).not.toHaveBeenCalled();
  expect(f.calls.cancelled).not.toHaveBeenCalled();
});
