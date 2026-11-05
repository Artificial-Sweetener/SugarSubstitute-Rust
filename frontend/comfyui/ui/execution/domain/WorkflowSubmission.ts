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
/** Describe captured workflow ownership independently of host prompt object lifetimes. */

import type { WorkflowSessionContext } from './WorkflowSession.js';
import type { WorkflowOutputSource } from './WorkflowOutput.js';
import type { WorkflowSessionId } from './ExecutionIdentity.js';
import type { WorkflowCanvasReturn } from './WorkflowPreparation.js';
import { freezeWorkflowSceneMember, type WorkflowSceneMember } from './WorkflowScene.js';

export interface WorkflowSubmissionOrderSource {
  /** Read reserved queue order even while its transport receipt is outstanding. */
  latestOrder(session: WorkflowSessionId): number;
}

export interface WorkflowSubmissionContext {
  readonly session: WorkflowSessionContext;
  readonly sources: readonly WorkflowOutputSource[];
  readonly canvasReturns?: readonly WorkflowCanvasReturn[];
  /** The prompt scene this submission renders, when its request queued one prompt per scene. */
  readonly scene?: WorkflowSceneMember;
}

export interface WorkflowOrderedSubmission extends WorkflowSubmissionContext {
  readonly order: number;
}

/** Copy ownership metadata so neither callers nor host mutations can change accepted routing. */
export function freezeWorkflowSubmissionContext(
  context: WorkflowSubmissionContext,
): WorkflowSubmissionContext {
  return Object.freeze({
    ...(context.scene ? { scene: freezeWorkflowSceneMember(context.scene) } : {}),
    ...(context.canvasReturns
      ? {
          canvasReturns: Object.freeze(
            context.canvasReturns.map((value) =>
              value.kind === 'unavailable'
                ? Object.freeze({ ...value })
                : Object.freeze({
                    ...value,
                    document: Object.freeze({ ...value.document }),
                    regions: Object.freeze(
                      value.regions.map((region) =>
                        Object.freeze({
                          ...region,
                          mask: Object.freeze({ ...region.mask }),
                          mapping: Object.freeze({
                            imageSize: Object.freeze({ ...region.mapping.imageSize }),
                            imageBounds: Object.freeze({ ...region.mapping.imageBounds }),
                            documentBounds: Object.freeze({ ...region.mapping.documentBounds }),
                          }),
                        }),
                      ),
                    ),
                  }),
            ),
          ),
        }
      : {}),
    session: Object.freeze({ ...context.session }),
    sources: Object.freeze(
      context.sources.map((source) =>
        Object.freeze({
          ...source,
          breadcrumb: Object.freeze([...source.breadcrumb]),
        }),
      ),
    ),
  });
}
