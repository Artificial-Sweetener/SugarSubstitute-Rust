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
import type { WorkflowScene } from './WorkflowScene.js';

/** One scene of the current scene run with the latest output of each source it produced. */
export interface WorkflowSceneResult extends WorkflowScene {
  readonly outputs: readonly WorkflowOutputSet[];
}

/** The scene run whose prompts produced the workflow's current results. */
export interface WorkflowSceneResults {
  readonly id: string;
  readonly scenes: readonly WorkflowSceneResult[];
  /** The scene of the latest known run, whose preview is the live one. */
  readonly running: string | null;
}

export interface WorkflowResultSession {
  readonly context: WorkflowSessionContext;
  readonly revision: number;
  /** The latest output of each source across every run, including every scene. */
  readonly outputs: readonly WorkflowOutputSet[];
  /** Per-scene outputs while the current results came from one scene run, or null. */
  readonly scenes: WorkflowSceneResults | null;
  readonly runOrder: number;
  /** Keep the accepted visual handoff monotonic even after transient previews retire. */
  readonly presentedRunOrder: number;
  readonly runStatus: WorkflowRunStatus | 'idle';
  readonly preview: WorkflowPreview | null;
  readonly recovery: {
    readonly state: 'idle' | 'loading' | 'ready' | 'unavailable';
    readonly message: string;
  };
}
