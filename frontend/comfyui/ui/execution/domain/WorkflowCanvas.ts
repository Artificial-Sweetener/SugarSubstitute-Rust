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
/** Describe canvas deliveries without exposing browser pixels, native documents or transports. */
import type { CanvasAdapter } from '../../../../../integrations/contracts/CanvasAdapter.js';
import type { WorkflowOrderedSubmission } from './WorkflowSubmission.js';
import type { WorkflowRun } from './WorkflowRun.js';
import type { WorkflowMediaReference } from './WorkflowMedia.js';
import type { WorkflowOutputSource } from './WorkflowOutput.js';
import type { WorkflowResultSession } from './WorkflowResultSession.js';
import type { WorkflowSessionId } from './ExecutionIdentity.js';
import type { WorkflowPreview } from './WorkflowPreview.js';

export interface WorkflowCanvasImage {
  readonly id: string;
  readonly media: WorkflowMediaReference;
}
export interface WorkflowCanvasOutput {
  readonly run: WorkflowRun;
  readonly source: WorkflowOutputSource;
  readonly images: readonly WorkflowCanvasImage[];
  readonly state: WorkflowCanvasState | null;
}
export interface WorkflowCanvasState {
  readonly sessionId: WorkflowSessionId;
  readonly result: WorkflowResultSession | null;
}
export interface WorkflowCanvasPreview<Bytes> {
  readonly source: { readonly run: WorkflowRun; readonly source: WorkflowOutputSource };
  readonly preview: WorkflowPreview;
  readonly bytes: Bytes;
}
export type WorkflowCanvasAdapter<Bytes> = CanvasAdapter<
  WorkflowOrderedSubmission,
  WorkflowCanvasOutput,
  WorkflowCanvasState,
  WorkflowCanvasPreview<Bytes>
>;

/** Expose a canvas's result projection without sharing its execution subscriptions. */
export interface WorkflowCanvasResults {
  read(sessionId: WorkflowSessionId): WorkflowResultSession | null;
  subscribe(listener: (sessionId: WorkflowSessionId) => void): () => void;
}
