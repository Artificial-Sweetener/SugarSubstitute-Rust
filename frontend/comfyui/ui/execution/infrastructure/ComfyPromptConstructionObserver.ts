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
/** Capture session ownership before Comfy awaits widgets or changes the active workflow. */

import { isRecord } from '../../types/common.js';
import type { WorkflowSessionStore } from '../application/WorkflowSessionStore.js';
import type { WorkflowSubmissionContexts } from '../application/WorkflowSubmissionContexts.js';
import type { WorkflowSessionContext } from '../domain/WorkflowSession.js';
import { ComfyAsyncMethodObserver } from './ComfyAsyncMethodObserver.js';
import { decodeComfyPromptSources } from './ComfyPromptSourceDecoder.js';

/** Observe construction only; preserve result identity and leave every node callback untouched. */
export function createComfyPromptConstructionObserver(
  app: unknown,
  sessions: WorkflowSessionStore,
  contexts: WorkflowSubmissionContexts,
  logger: Pick<Console, 'error'>,
): ComfyAsyncMethodObserver<WorkflowSessionContext | null> {
  return new ComfyAsyncMethodObserver({
    host: app,
    method: 'graphToPrompt',
    logger,
    enter: () => sessions.read().active,
    succeeded: (session, result) => {
      if (session && isRecord(result))
        contexts.associate(result, session, decodeComfyPromptSources(result));
    },
    failed: () => undefined,
    cancelled: () => undefined,
  });
}
