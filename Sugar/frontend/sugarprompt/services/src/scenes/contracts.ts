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
/** Hosts supply a detached prompt endpoint view; workflow graph discovery and editable source stay with their owners. */

/** A stable host endpoint identity has one stack container and semantic role; null source represents an absent/non-text field. */
export interface WorkflowScenePrompt {
  readonly id: string;
  readonly stackId: string;
  readonly role: 'positive' | 'negative';
  readonly linkable: boolean;
  readonly source: string | null;
}

/** One closed workflow publication names stack order separately from endpoint discovery order. */
export interface WorkflowSceneInput {
  readonly revision: string;
  readonly stackOrder: readonly string[];
  readonly prompts: readonly WorkflowScenePrompt[];
}

/** The first authored occurrence supplies normalized identity, literal title and contiguous workflow order. */
export interface WorkflowScene {
  readonly key: string;
  readonly title: string;
  readonly order: number;
}

/** Every linkable endpoint is checked against authority; keys are normalized by the core scene parser. */
export interface WorkflowSceneDiagnostics {
  readonly promptId: string;
  readonly orphanKeys: readonly string[];
  readonly duplicateKeys: readonly string[];
}

/** Invalid authored blocks prevent scene generation instead of silently discarding duplicate/orphan content. */
export type WorkflowSceneGeneration =
  | { readonly status: 'ready' }
  | { readonly status: 'no-authority' | 'no-scenes' }
  | {
      readonly status: 'invalid-scenes';
      readonly promptIds: readonly string[];
    };

/** Immutable derived workflow metadata retains no prompt text or mutable graph; hosts retire it when its revision changes. */
export interface WorkflowSceneAnalysis {
  readonly revision: string;
  readonly authorityId: string | null;
  readonly scenes: readonly WorkflowScene[];
  readonly diagnostics: readonly WorkflowSceneDiagnostics[];
  readonly generation: WorkflowSceneGeneration;
}
