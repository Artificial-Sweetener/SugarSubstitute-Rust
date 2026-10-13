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
/** Prove presentation restoration follows committed workflow sessions and tolerates storage failure. */
import { jest } from '@jest/globals';
import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';
import { workflowSessionId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import {
  ComfyWorkflowViewPreferences,
  WORKFLOW_VIEW_STORAGE_KEY,
} from '../../../frontend/comfyui/ui/workflow/infrastructure/ComfyWorkflowViewPreferences.js';

test('restores both choices atomically after activation and performs zero reads/writes on repeated switches', () => {
  const sessions = new WorkflowSessionStore();
  const a = { sessionId: workflowSessionId('a'), title: 'A', path: 'workflows/a.json' };
  const b = { sessionId: workflowSessionId('b'), title: 'B', path: 'workflows/b.json' };
  const storage = {
    getItem: jest.fn(() => JSON.stringify([{ path: a.path, canvas: false, masonry: true }])),
    setItem: jest.fn(),
  };
  sessions.synchronize([a, b], a.sessionId);
  const views = new ComfyWorkflowViewPreferences(
    sessions,
    () => storage,
    () => true,
    { warn: jest.fn() },
  );
  views.setup();
  expect(views.snapshot).toEqual({ canvas: false, masonry: true });
  const listener = jest.fn(() => ({ ready: views.ready, ...views.snapshot }));
  views.subscribe(listener);
  const token = sessions.beginLoad();
  sessions.synchronize([a, b], b.sessionId);
  expect(views.ready).toBe(false);
  expect(views.snapshot).toEqual({ canvas: false, masonry: true });
  views.change({ canvas: true });
  expect(storage.setItem).not.toHaveBeenCalled();
  sessions.completeLoad(token, true);
  expect(views.ready).toBe(true);
  expect(views.snapshot).toEqual({ canvas: true, masonry: false });
  expect(listener).toHaveBeenCalledTimes(2);
  views.change({ canvas: false, masonry: true });
  expect(storage.setItem).toHaveBeenCalledWith(WORKFLOW_VIEW_STORAGE_KEY, expect.any(String));
  storage.setItem.mockClear();
  for (let index = 0; index < 100; index++) {
    const load = sessions.beginLoad();
    sessions.synchronize([a, b], index % 2 === 0 ? a.sessionId : b.sessionId);
    sessions.completeLoad(load, true);
    expect(views.snapshot).toEqual({ canvas: false, masonry: true });
  }
  expect(storage.getItem).toHaveBeenCalledTimes(1);
  expect(storage.setItem).not.toHaveBeenCalled();
  views.dispose();
});

test('does not let failed loads or unavailable storage discard accepted user choices', () => {
  const sessions = new WorkflowSessionStore();
  const a = { sessionId: workflowSessionId('a'), title: 'A', path: 'workflows/a.json' };
  sessions.synchronize([a], a.sessionId);
  const logger = { warn: jest.fn() };
  const views = new ComfyWorkflowViewPreferences(
    sessions,
    () => {
      throw new Error('Storage denied');
    },
    () => true,
    logger,
  );
  views.setup();
  views.change({ canvas: false, masonry: true });
  expect(logger.warn).toHaveBeenCalledTimes(3);
  expect(views.snapshot).toEqual({ canvas: false, masonry: true });
  const token = sessions.beginLoad();
  sessions.completeLoad(token, false);
  expect(views.ready).toBe(false);
  expect(views.snapshot).toEqual({ canvas: false, masonry: true });
  views.dispose();
});
