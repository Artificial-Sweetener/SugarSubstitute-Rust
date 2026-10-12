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
/** Translate queue observations into validated accepted submission identities. */

import { isRecord } from '../../types/common.js';
import { promptId, type PromptId } from '../domain/ExecutionIdentity.js';
import type { WorkflowSubmissionContext } from '../domain/WorkflowSubmission.js';
import type { WorkflowSubmissionContexts } from '../application/WorkflowSubmissionContexts.js';
import type { PromptQueueObservation, PromptQueueObserver } from './ComfyPromptQueueBridge.js';
import { decodeComfyPromptSources } from './ComfyPromptSourceDecoder.js';

export interface ComfyPromptSubmissionObserverOptions {
  readonly contexts: WorkflowSubmissionContexts;
  readonly logger: Pick<Console, 'error' | 'warn'>;
  accepted(context: WorkflowSubmissionContext, id: PromptId): void;
}

/** Resolve only observed or explicitly associated prompts and contain diagnostic failures in the bridge. */
export class ComfyPromptSubmissionObserver implements PromptQueueObserver {
  readonly logger: Pick<Console, 'error' | 'warn'>;

  /** Bind typed association publication rather than introducing a second queue interceptor. */
  constructor(private readonly options: ComfyPromptSubmissionObserverOptions) {
    this.logger = options.logger;
  }

  /** Capture immutable entry-time ownership before preparation or transformation can await. */
  capture(payload: unknown): PromptQueueObservation | null {
    if (!isRecord(payload)) return null;
    const captured = this.options.contexts.read(payload);
    if (!captured) return null;
    let context = captured;
    let active = true;
    return {
      prepared: (prepared) => {
        if (!active) return;
        const sources = decodeComfyPromptSources(prepared);
        if (sources.length) context = Object.freeze({ session: captured.session, sources });
      },
      accepted: (response) => {
        if (!active) return;
        active = false;
        const id = isRecord(response) ? response.prompt_id : null;
        if (typeof id !== 'string' || !id.trim() || id.length > 4096) {
          this.logger.warn(
            'SugarCubes cannot associate a queue response without a valid prompt ID.',
            {
              sessionId: context.session.sessionId,
            },
          );
          return;
        }
        this.options.accepted(context, promptId(id));
      },
      rejected: () => {
        active = false;
      },
      cancelled: () => {
        active = false;
      },
    };
  }
}
