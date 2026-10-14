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
/** Describe a lightweight workflow inspection catalog without allocating image resources. */

import type {
  WorkflowOutputItem,
  WorkflowOutputSource,
} from '../../execution/domain/WorkflowOutput.js';
import type { WorkflowPreview } from '../../execution/domain/WorkflowPreview.js';
import type { WorkflowResultSession } from '../../execution/domain/WorkflowResultSession.js';
import type { CanvasSourceSummary } from './CanvasViewPort.js';
import type { CanvasSessionId } from './CanvasIdentity.js';

export interface CanvasCatalogEntry {
  readonly source: WorkflowOutputSource;
  readonly summary: CanvasSourceSummary;
  readonly items: readonly WorkflowOutputItem[];
  readonly preview: WorkflowPreview | null;
}

export interface CanvasCatalogSnapshot {
  readonly sessionId: CanvasSessionId;
  readonly generation: number;
  readonly sources: readonly WorkflowOutputSource[];
  readonly entries: readonly CanvasCatalogEntry[];
  readonly result: WorkflowResultSession;
}
