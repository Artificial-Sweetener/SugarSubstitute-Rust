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
/** Prove only waiting queue cards lift, and no queue card offers a text cursor or selection. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { GenerationQueueReorder } from '../../../frontend/comfyui/ui/surface/frame/queue/GenerationQueueReorder.js';
import { generationQueueViewCss } from '../../../frontend/comfyui/ui/surface/frame/queue/GenerationQueueViewStyles.js';

afterEach(() => document.body.replaceChildren());

function pointer(type: string, y: number): PointerEvent {
  const event = new MouseEvent(type, { bubbles: true, cancelable: true, clientX: 50, clientY: y });
  Object.defineProperty(event, 'pointerId', { value: 1 });
  Object.defineProperty(event, 'button', { value: 0 });
  return event as PointerEvent;
}

/** A waiting job above a finished one that may only be carried to the tab bar. */
function queue() {
  const root = document.createElement('div');
  const rows = document.createElement('div');
  rows.setPointerCapture = jest.fn();
  rows.releasePointerCapture = jest.fn();
  rows.hasPointerCapture = jest.fn(() => true);
  rows.getBoundingClientRect = () =>
    ({ left: 0, top: 0, right: 300, bottom: 200, width: 300, height: 200 }) as DOMRect;
  const row = (jobId: string, role: string, top: number): HTMLElement => {
    const element = document.createElement('div');
    element.className = 'sc-generation-queue-row';
    element.dataset.jobId = jobId;
    element.dataset.role = role;
    element.getBoundingClientRect = () =>
      ({ left: 0, top, right: 300, bottom: top + 68, width: 300, height: 68 }) as DOMRect;
    rows.append(element);
    return element;
  };
  const waiting = row('waiting', 'waiting', 0);
  const finished = row('finished', 'finished', 74);
  root.append(rows);
  document.body.append(root);
  const reorder = new GenerationQueueReorder({
    root,
    rows,
    waiting: () => ['waiting'],
    busy: () => false,
    preview: () => undefined,
    commit: () => undefined,
    carries: (jobId) => jobId === 'finished',
  });
  const drag = (element: HTMLElement, from: number): void => {
    element.dispatchEvent(pointer('pointerdown', from));
    document.dispatchEvent(pointer('pointermove', from + 30));
    document.dispatchEvent(pointer('pointermove', from + 40));
  };
  return { root, rows, waiting, finished, reorder, drag };
}

test('a waiting job lifts off the list and leaves a placeholder', () => {
  const q = queue();
  q.drag(q.waiting, 30);
  expect(q.root.querySelector('.sc-generation-queue-lifted')).not.toBeNull();
  expect(q.waiting.hasAttribute('data-dragging')).toBe(true);
  q.reorder.dispose();
});

test('a finished job never lifts and leaves no placeholder, since it cannot be rearranged', () => {
  const q = queue();
  q.drag(q.finished, 100);
  expect(q.root.querySelector('.sc-generation-queue-lifted')).toBeNull();
  expect(q.finished.hasAttribute('data-dragging')).toBe(false);
  expect(q.rows.hasAttribute('data-reordering')).toBe(false);
  q.reorder.dispose();
});

test('queue cards show the arrow and no text selection, and only waiting cards show the grab hand', () => {
  const css = generationQueueViewCss.replace(/\s+/g, ' ');
  expect(css).toContain(
    '.sc-generation-queue-row { cursor:default; user-select:none; -webkit-user-select:none; }',
  );
  expect(css).toContain(".sc-generation-queue-row[data-role='waiting'] { cursor:grab;");
  expect(css).not.toMatch(/cursor:text/);
});
