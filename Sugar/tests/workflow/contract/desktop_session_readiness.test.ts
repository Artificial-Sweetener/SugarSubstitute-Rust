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
/** Prove startup waits for built-in persistence and committed graph loading without polling. */
import { ComfyDesktopReadiness } from '../../../frontend/comfyui/ui/workflow/infrastructure/ComfyDesktopReadiness.js';
import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';
import { workflowSessionId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';

test('extension setup returns while startup is loading, then commitment and explicit opt-out are observable', () => {
  const sessions = new WorkflowSessionStore();
  const listeners = new Set<() => void>();
  const settingListeners = new Set<() => void>();
  let persist = true;
  const manager = {
    spinner: true,
    $subscribe(listener: () => void) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
  const app = {
    extensionManager: manager,
    ui: {
      settings: {
        getSettingValue: () => persist,
        addEventListener(_event: string, listener: () => void) {
          settingListeners.add(listener);
        },
        removeEventListener(_event: string, listener: () => void) {
          settingListeners.delete(listener);
        },
      },
    },
  };
  const readiness = new ComfyDesktopReadiness(app, sessions);
  readiness.install();
  const a = { sessionId: workflowSessionId('A'), title: 'A', path: '' };
  const load = sessions.beginLoad();
  sessions.synchronize([a], a.sessionId);
  expect(readiness.read()).toEqual({ committed: false, workflowPersistence: true });
  sessions.completeLoad(load, true);
  expect(readiness.read().committed).toBe(false);
  manager.spinner = false;
  for (const listener of listeners) listener();
  expect(readiness.read().committed).toBe(true);
  persist = false;
  for (const listener of settingListeners) listener();
  expect(readiness.read()).toEqual({ committed: true, workflowPersistence: false });
  const delayed = sessions.beginLoad('activate');
  expect(readiness.read().committed).toBe(false);
  sessions.completeLoad(delayed, true);
  expect(readiness.read().committed).toBe(true);
  readiness.dispose();
  expect(listeners.size).toBe(0);
  expect(settingListeners.size).toBe(0);
});
