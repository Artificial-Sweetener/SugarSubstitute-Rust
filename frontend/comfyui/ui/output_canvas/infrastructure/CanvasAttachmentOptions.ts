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
/** Supply shared workflow and overlay services to either output presenter. */

import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { ExecutionClock } from '../../execution/domain/WorkflowExecutionEvent.js';
import type { CanvasFrameClock } from '../application/CanvasCameraTransition.js';
import type { CanvasInspectionStore } from '../application/CanvasInspectionStore.js';
import type { CanvasSelectionController } from '../application/CanvasSelectionController.js';
import type { CanvasViewRegistry } from '../application/CanvasViewRegistry.js';
import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import type { CanvasAttachmentAddress } from '../domain/CanvasIdentity.js';
import type { CanvasCommandRequest } from '../domain/CanvasViewPort.js';
import type { CanvasOutputDragPayload } from '../presentation/CanvasOutputDrag.js';
import type { CanvasMediaOptions } from './CanvasMediaActions.js';
import type { CanvasRoutePort } from '../domain/CanvasRoute.js';

/** Keep workflow ownership and transfer actions independent of image presentation. */
export interface CanvasAttachmentOptions {
  readonly address: CanvasAttachmentAddress;
  readonly views: CanvasViewRegistry;
  readonly selection: CanvasSelectionController;
  readonly inspections: CanvasInspectionStore;
  readonly clock: ExecutionClock;
  readonly frames: CanvasFrameClock;
  /** Read the authoritative active catalog only at result/navigation boundaries. */
  catalog(): CanvasCatalogSnapshot | null;
  /** Check the fixed attachment and workflow generation without graph traversal. */
  accepts(): boolean;
  /** Read shared connection state without installing another execution listener. */
  connected(): boolean;
  copy?: CanvasMediaOptions['copy'];
  fileAction?: CanvasMediaOptions['fileAction'];
  mediaProfile?: CanvasMediaOptions['profile'];
  photoshop?: CanvasMediaOptions['photoshop'];
  /** Prepare one exact completed image for the page's outbound drag. */
  drag?(item: WorkflowOutputItem, signal: AbortSignal): Promise<CanvasOutputDragPayload | null>;
  /** Invalidate prepared transfers after an accepted shared preference change. */
  transferChanged?(listener: () => void): () => void;
  /** Show the Input/Output route when a workflow-surface canvas holds the workflow's inputs. */
  route?: CanvasRoutePort;
  /** Report product feedback through the existing SugarCubes services. */
  report(error: unknown, request: CanvasCommandRequest | null): void;
}
