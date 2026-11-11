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
/** Prove copied widget refresh is scoped to exact node identity and an explicit host reset. */
import { expect, jest, test } from '@jest/globals';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';
import {
  refreshNativeNodeCards,
  subscribeNativeNodeCardRefresh,
} from '../../../frontend/comfyui/ui/surface/NativeNodeCardRefresh.js';

test('captures every affected card before any scoped native render and releases both phases', () => {
  const first: ComfyNode = { id: 1 };
  const second: ComfyNode = { id: 2 };
  const unrelated: ComfyNode = { id: 3 };
  const events: string[] = [];
  const release = subscribeNativeNodeCardRefresh(
    document,
    [first],
    () => events.push('render first'),
    () => events.push('measure first'),
  );
  const releaseSecond = subscribeNativeNodeCardRefresh(
    document,
    [second],
    () => events.push('render second'),
    () => events.push('measure second'),
  );
  const releaseUnrelated = subscribeNativeNodeCardRefresh(
    document,
    [unrelated],
    () => events.push('unrelated'),
    () => events.push('measure unrelated'),
  );
  refreshNativeNodeCards(document, { kind: 'nodes', nodes: [first, second] });
  expect(events).toEqual(['measure first', 'measure second', 'render first', 'render second']);
  release();
  releaseSecond();
  releaseUnrelated();
  events.length = 0;
  refreshNativeNodeCards(document, { kind: 'reset', reason: 'native-choices' });
  expect(events).toEqual([]);
});
test('refreshes owning mounts only, preserves forwarded ownership and rejects malformed events', () => {
  const a: ComfyNode = { id: 1 },
    b: ComfyNode = { id: 2 },
    forwarded: ComfyNode = { id: 3 };
  const first = jest.fn(),
    second = jest.fn();
  const release = subscribeNativeNodeCardRefresh(document, [a, forwarded], first);
  const releaseSecond = subscribeNativeNodeCardRefresh(document, [b], second);
  refreshNativeNodeCards(document, { kind: 'nodes', nodes: [forwarded] });
  expect(first).toHaveBeenCalledTimes(1);
  expect(second).not.toHaveBeenCalled();
  document.dispatchEvent(
    new CustomEvent('sugarcubes-native-node-card-change', { detail: { kind: 'nodes' } }),
  );
  expect(first).toHaveBeenCalledTimes(1);
  refreshNativeNodeCards(document, { kind: 'reset', reason: 'native-choices' });
  expect(first).toHaveBeenCalledTimes(2);
  expect(second).toHaveBeenCalledTimes(1);
  release();
  releaseSecond();
  refreshNativeNodeCards(document, { kind: 'nodes', nodes: [a, b] });
  expect(first).toHaveBeenCalledTimes(2);
  expect(second).toHaveBeenCalledTimes(1);
});
