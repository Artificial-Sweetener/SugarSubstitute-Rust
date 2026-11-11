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
/** Describe generation lifecycle transitions independently of result viewers and native editors. */
import type { PromptId, WorkflowSessionId } from './ExecutionIdentity.js';
import type { WorkflowOrderedSubmission } from './WorkflowSubmission.js';
import type { WorkflowRun } from './WorkflowRun.js';
import type { WorkflowMediaPayload } from './WorkflowMedia.js';
import type { WorkflowOutputSource } from './WorkflowOutput.js';

export type WorkflowGenerationEvent =
  | { readonly type: 'submitted'; readonly submission: WorkflowOrderedSubmission }
  | { readonly type: 'rejected'; readonly submission: WorkflowOrderedSubmission }
  | {
      readonly type: 'accepted';
      readonly submission: WorkflowOrderedSubmission;
      readonly run: WorkflowRun;
    }
  | {
      readonly type: 'output';
      readonly run: WorkflowRun;
      readonly source: WorkflowOutputSource;
      readonly payload: WorkflowMediaPayload;
      readonly retained: boolean;
    }
  | { readonly type: 'terminal'; readonly run: WorkflowRun }
  | { readonly type: 'settled'; readonly promptId: PromptId }
  | { readonly type: 'closed'; readonly sessionId: WorkflowSessionId }
  | { readonly type: 'disposed' };
