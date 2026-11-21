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
/** Prove every way of putting the pinned queue away slides it out before it goes. */
import { expect, jest, test } from '@jest/globals';
import { ComfyQueueActions } from '../../../frontend/comfyui/ui/surface/frame/queue/ComfyQueueActions.js';

/** A ComfyUI whose pinned queue is open, with the order in which things happen to it. */
function pinned() {
  const events: string[] = [];
  const stored: Record<string, unknown> = { 'Comfy.Queue.QPOV2': true };
  const tabs = {
    activeSidebarTabId: 'job-history' as string | null,
    toggleSidebarTab: (id: string) => {
      events.push(`toggle ${id}`);
      tabs.activeSidebarTabId = tabs.activeSidebarTabId === id ? null : id;
    },
  };
  const app = {
    extensionManager: {
      sidebarTab: tabs,
      setting: {
        get: (id: string) => stored[id],
        set: async (id: string, value: unknown) => {
          events.push(`set ${id}=${String(value)}`);
          stored[id] = value;
        },
      },
    },
  };
  let finishSlide = (): void => undefined;
  const leave = jest.fn((then: () => void) => {
    events.push('slide out');
    finishSlide = then;
  });
  const actions = new ComfyQueueActions(
    { fetchApi: jest.fn() } as unknown as ConstructorParameters<typeof ComfyQueueActions>[0],
    app,
    () => undefined,
    leave,
  );
  return { actions, events, leave, finish: () => finishSlide() };
}

test('putting the pinned queue away slides it out, then closes its panel', () => {
  const queue = pinned();
  queue.actions.closePanel();
  expect(queue.events).toEqual(['slide out']);
  queue.finish();
  expect(queue.events).toEqual(['slide out', 'toggle job-history']);
});

test('unpinning the queue slides it out before the setting removes its panel', async () => {
  const queue = pinned();
  const unpinned = queue.actions.setDocked(false);
  await Promise.resolve();
  expect(queue.events).toEqual(['slide out']);
  queue.finish();
  await unpinned;
  expect(queue.events[0]).toBe('slide out');
  expect(queue.events.slice(1).every((event) => event.startsWith('set '))).toBe(true);
  expect(queue.events).toHaveLength(2);
});
