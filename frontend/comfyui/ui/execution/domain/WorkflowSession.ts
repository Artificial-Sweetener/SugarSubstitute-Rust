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
/** Describe workflow lifetimes independently of reused host graphs and tab paths. */

import type { WorkflowSessionId } from './ExecutionIdentity.js';

export interface WorkflowSessionBinding {
  readonly sessionId: WorkflowSessionId;
  readonly title: string;
  readonly path: string;
}

export interface WorkflowSessionContext extends WorkflowSessionBinding {
  readonly generation: number;
}

export interface WorkflowSessionSnapshot {
  readonly revision: number;
  readonly open: readonly WorkflowSessionContext[];
  readonly active: WorkflowSessionContext | null;
  readonly loading: boolean;
}

export interface WorkflowLoadToken {
  readonly sequence: number;
  readonly previousSessionId: WorkflowSessionId | null;
}
