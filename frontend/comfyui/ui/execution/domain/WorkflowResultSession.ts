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
/** Publish one workflow's latest lightweight results and execution frontier. */

import type { WorkflowSessionContext } from './WorkflowSession.js';
import type { WorkflowOutputSet, WorkflowRunStatus } from './WorkflowOutput.js';
import type { WorkflowPreview } from './WorkflowPreview.js';

export interface WorkflowResultSession {
  readonly context: WorkflowSessionContext;
  readonly revision: number;
  readonly outputs: readonly WorkflowOutputSet[];
  readonly runOrder: number;
  readonly runStatus: WorkflowRunStatus | 'idle';
  readonly preview: WorkflowPreview | null;
  readonly recovery: {
    readonly state: 'idle' | 'loading' | 'ready' | 'unavailable';
    readonly message: string;
  };
}
