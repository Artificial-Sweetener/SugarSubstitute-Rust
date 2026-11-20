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

import {
  outputSourceId,
  type OutputResultId,
  type OutputSourceId,
  type PromptId,
  type WorkflowSessionId,
} from './ExecutionIdentity.js';
import type {
  SessionBinding,
  SessionOutput,
} from '../../../../../integrations/contracts/SessionProtocol.generated.js';

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
    | {
        readonly kind: 'desktop-session';
        readonly promptId: PromptId | null;
        readonly snapshotId: string;
        readonly entryId: string;
        readonly runId: string;
      }
  );

export interface WorkflowOutputItem {
  readonly id: OutputResultId;
  readonly provenance: OutputProvenance;
  readonly batchIndex: number;
  readonly file: OutputFileReference;
  readonly dimensions: ImageDimensions | null;
  readonly mediaKind: WorkflowMediaKind;
  /** Carry a revocable current-page retained-asset URL for presentation; file provenance stays independent. */
  readonly presentationUrl?: string;
  /** Record a byte-verified encoded MIME when artifact resolution supplies it. */
  readonly encodedMime?: string;
  /** Keep verified restored artifact facts distinct from the current backend file descriptor. */
  readonly artifact?: { readonly output: SessionOutput; readonly binding: SessionBinding };
  /** Preserve exact final membership when its bytes cannot currently be resolved. */
  readonly unavailable?: { readonly code: string; readonly message: string };
}

export type WorkflowMediaKind = 'image' | 'animated' | 'video' | 'model' | 'unsupported';

/** What a workflow says about the Cube one output source belongs to. */
export interface CubeOutputFacts {
  /** The Cube's title as authored, model folder included. */
  readonly title: string;
  /** The Cube's own name: its title without the folder of the model family it targets. */
  readonly name: string;
  readonly outputName: string;
  /** How many outputs the Cube exposes. */
  readonly outputCount: number;
}

export interface WorkflowOutputSource {
  readonly id: OutputSourceId;
  readonly executionPath: string;
  readonly label: string;
  readonly breadcrumb: readonly string[];
  readonly order: number;
  /** Present on a Cube's output, so every presenter names it by the same rule. */
  readonly cube?: CubeOutputFacts;
  /** Route intermediate execution frames without treating their other media as the bound final. */
  readonly transientOnly?: true;
}

/**
 * Reduce execution-path attributions to one entry per output source.
 *
 * A Cube routes every node inside it to its exposed output so their previews appear there, which
 * gives several paths one source identity. Only the path that delivers the source's finals
 * describes the source itself: the path the identity names, else a path whose finals it accepts.
 */
export function finalOutputSources(
  values: readonly WorkflowOutputSource[],
): readonly WorkflowOutputSource[] {
  const chosen = new Map<OutputSourceId, WorkflowOutputSource>();
  for (const value of values) {
    const current = chosen.get(value.id);
    if (!current || finalPathRank(value) < finalPathRank(current)) chosen.set(value.id, value);
  }
  return Object.freeze([...chosen.values()]);
}

/** Order one source's paths by how directly each delivers its final media. */
function finalPathRank(source: WorkflowOutputSource): number {
  if (source.id === outputSourceId(source.executionPath)) return 0;
  return source.transientOnly ? 2 : 1;
}

export interface WorkflowOutputSet {
  readonly source: WorkflowOutputSource;
  readonly provenance: OutputProvenance;
  readonly items: readonly WorkflowOutputItem[];
  readonly text: readonly string[];
  readonly batchCount: number;
}

export type WorkflowRunStatus = 'queued' | 'running' | 'complete' | 'cancelled' | 'error';
