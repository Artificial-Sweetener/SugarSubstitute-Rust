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
/** Commit session attribution after the host completes graph loading and workflow activation. */

import type { WorkflowSessionStore } from '../application/WorkflowSessionStore.js';
import { ComfyAsyncMethodObserver } from './ComfyAsyncMethodObserver.js';
import type { ComfyWorkflowSessionAdapter } from './ComfyWorkflowSessionAdapter.js';

/** Observe the public load boundary across tab switches, same-object reload and undo restoration. */
export function createComfyWorkflowLoadObserver(
  app: unknown,
  sessions: WorkflowSessionStore,
  adapter: ComfyWorkflowSessionAdapter,
  logger: Pick<Console, 'error'>,
): ComfyAsyncMethodObserver<ReturnType<WorkflowSessionStore['beginLoad']>> {
  return new ComfyAsyncMethodObserver({
    host: app,
    method: 'loadGraphData',
    logger,
    enter: () => sessions.beginLoad(),
    succeeded: (token) => {
      sessions.completeLoad(token, adapter.refresh());
    },
    failed: (token, error) => {
      sessions.completeLoad(token, false);
      logger.error('SugarCubes workflow graph load failed; output association is suspended.', {
        error,
      });
    },
    cancelled: (token) => sessions.completeLoad(token, false),
  });
}
