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
/** Describe attributable workflow media without decoded images or host values. */

import type {
  OutputResultId,
  OutputSourceId,
  PromptId,
  WorkflowSessionId,
} from './ExecutionIdentity.js';

export interface ImageDimensions {
  readonly width: number;
  readonly height: number;
}

export interface OutputFileReference {
  readonly filename: string;
  readonly subfolder: string;
  readonly type: 'input' | 'output' | 'temp';
}

export interface OutputProvenanceContext {
  readonly sessionId: WorkflowSessionId;
  readonly sourceId: OutputSourceId;
  readonly executionPath: string;
  readonly generation: number;
  readonly runOrder: number;
  readonly sourceOrder: number;
  readonly revision: number;
}

export type OutputProvenance = OutputProvenanceContext &
  (
    | { readonly kind: 'execution'; readonly promptId: PromptId }
    | { readonly kind: 'workflow-snapshot'; readonly promptId: null; readonly snapshotId: string }
  );

export interface WorkflowOutputItem {
  readonly id: OutputResultId;
  readonly provenance: OutputProvenance;
  readonly batchIndex: number;
  readonly file: OutputFileReference;
  readonly dimensions: ImageDimensions | null;
  readonly mediaKind: WorkflowMediaKind;
}

export type WorkflowMediaKind = 'image' | 'animated' | 'video' | 'unsupported';

export interface WorkflowOutputSource {
  readonly id: OutputSourceId;
  readonly executionPath: string;
  readonly label: string;
  readonly breadcrumb: readonly string[];
  readonly order: number;
  /** Route intermediate execution frames without treating their other media as the bound final. */
  readonly transientOnly?: true;
}

export interface WorkflowOutputSet {
  readonly source: WorkflowOutputSource;
  readonly provenance: OutputProvenance;
  readonly items: readonly WorkflowOutputItem[];
  readonly text: readonly string[];
  readonly batchCount: number;
}

export type WorkflowRunStatus = 'queued' | 'running' | 'complete' | 'cancelled' | 'error';
