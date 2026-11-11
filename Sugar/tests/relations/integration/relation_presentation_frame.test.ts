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
/** Verify native surface bursts share one presentation frame without deferring relation state. */
import { jest, test, expect } from '@jest/globals';
import { RelationHostLifecycle } from '../../../frontend/comfyui/ui/relations/RelationHostLifecycle.js';
import { CubeRelationPresenter } from '../../../frontend/comfyui/ui/relations/presentation/CubeRelationPresenter.js';
import { WorkflowSeedHistory } from '../../../frontend/comfyui/ui/execution/application/WorkflowSeedHistory.js';
import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';

test('coalesces native mounts and relation notifications into one cancellable presentation frame', async () => {
  const frames = new Map<number, FrameRequestCallback>();
  let nextFrame = 0;
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    frames.set(++nextFrame, callback);
    return nextFrame;
  });
  jest.spyOn(window, 'cancelAnimationFrame').mockImplementation((id) => {
    frames.delete(id);
  });
  const cards = jest.spyOn(CubeRelationPresenter.prototype, 'reconcile');
  const host = new RelationHostLifecycle({
    seeds: new WorkflowSeedHistory(),
    sessions: new WorkflowSessionStore(),
    app: { graph: { _nodes: [], extra: {} } },
    document,
    logger: console,
    reportError: jest.fn(),
    getCardState: () => null,
  });
  try {
    host.setup();
    await settle();
    for (let index = 0; index < 3; index++) {
      const card = document.createElement('div');
      card.className = 'lg-node';
      document.body.append(card);
      host.relations.reconcile(false);
      await settle();
    }
    expect(cards).not.toHaveBeenCalled();
    expect(frames.size).toBe(1);
    for (let index = 0; index < 3; index++) {
      const pending = [...frames.values()];
      frames.clear();
      for (const frame of pending) frame(0);
    }
    expect(cards).toHaveBeenCalledTimes(1);
    host.relations.reconcile(false);
    expect(frames.size).toBe(1);
    host.beforeConfigureGraph();
    expect(frames.size).toBe(0);
  } finally {
    host.dispose();
    document.body.replaceChildren();
    jest.restoreAllMocks();
  }
});

/** Drain scoped host mutation deliveries before advancing browser presentation. */
async function settle(): Promise<void> {
  for (let index = 0; index < 6; index++) await Promise.resolve();
}
