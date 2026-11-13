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
/** Verify native replacement batches retain live ownership without redundant discovery. */
import { jest } from '@jest/globals';
import { WorkflowMasonryRemountQueue } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryRemountQueue.js';

test('shares the latest sources once across cards and deduplicates invalidation', async () => {
  const source = document.createElement('div');
  const sources = jest.fn(() => new Map([['a', source]]));
  const changed = jest.fn();
  const queue = new WorkflowMasonryRemountQueue(sources, changed);
  const obsolete = jest.fn(() => true);
  const first = jest.fn(() => true);
  const second = jest.fn(() => true);
  queue.request('a', obsolete);
  queue.request('a', first);
  queue.request('b', second);
  expect(sources).not.toHaveBeenCalled();
  await Promise.resolve();
  expect(sources).toHaveBeenCalledTimes(1);
  expect(obsolete).not.toHaveBeenCalled();
  expect(first).toHaveBeenCalledWith(source);
  expect(second).toHaveBeenCalledWith(null);
  expect(changed).toHaveBeenCalledTimes(1);
  queue.dispose();
});

test('does not refresh presenters for retired mounts and cancels disposed work', async () => {
  const sources = jest.fn(() => new Map<string, HTMLElement>());
  const changed = jest.fn();
  const queue = new WorkflowMasonryRemountQueue(sources, changed);
  queue.request('retired', () => false);
  await Promise.resolve();
  expect(changed).not.toHaveBeenCalled();
  sources.mockClear();
  const rebind = jest.fn(() => true);
  queue.request('pending', rebind);
  queue.dispose();
  queue.request('late', rebind);
  await Promise.resolve();
  expect(sources).not.toHaveBeenCalled();
  expect(rebind).not.toHaveBeenCalled();
});

test('discovers replacement sources afresh for each later batch', async () => {
  const old = document.createElement('div');
  const next = document.createElement('div');
  const sources = jest.fn(() => new Map([['a', old]]));
  const queue = new WorkflowMasonryRemountQueue(sources, jest.fn());
  const rebind = jest.fn(() => true);
  queue.request('a', rebind);
  await Promise.resolve();
  sources.mockReturnValue(new Map([['a', next]]));
  queue.request('a', rebind);
  await Promise.resolve();
  expect(rebind.mock.calls).toEqual([[old], [next]]);
  expect(sources).toHaveBeenCalledTimes(2);
  queue.dispose();
});

test('stops notifications when rebinding disposes the view', async () => {
  const changed = jest.fn();
  const queue = new WorkflowMasonryRemountQueue(() => new Map(), changed);
  queue.request('last', () => {
    queue.dispose();
    return true;
  });
  await Promise.resolve();
  expect(changed).not.toHaveBeenCalled();
});
