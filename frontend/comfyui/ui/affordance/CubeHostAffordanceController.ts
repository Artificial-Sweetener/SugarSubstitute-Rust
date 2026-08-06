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
/** Route Cube affordance intents to existing application and domain owners. */

import { isDraftCubeNode, requireCubeIdentity } from '../cube/node/ComfyCubeNodeFactory.js';
import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import { readCubeNodeAuthoringCandidate } from '../cube/node/CubeNodeAuthoringCandidate.js';
import { readInstanceId } from '../cube/node/CubeNodeCatalog.js';
import type { CubeNodeCatalog } from '../cube/node/CubeNodeCatalog.js';
import type { ConfirmDialog } from '../dialogs/ConfirmDialog.js';
import type { CubeEditorContextResolver } from '../surface/CubeEditorContextResolver.js';
import type { CubeEditorMetadataHud } from '../surface/CubeEditorMetadataHud.js';

interface CubeCreationSave {
  saveDraft(instanceId: string, candidate: object): Promise<unknown | null>;
}

interface ExistingCubeSave {
  save(options: { cubeIds: string[] }): Promise<{ status: string; message?: string }>;
}

interface AffordanceFeedback {
  push?(severity: string, summary: string, detail?: string): unknown;
}

interface MutableCubeGraph {
  _nodes: unknown[];
  remove(node: unknown): void;
}

/** Expose only the graph-bound collaborators needed by host affordances. */
export interface CubeAffordanceRuntime {
  contexts: CubeEditorContextResolver;
  metadataHud: CubeEditorMetadataHud | null;
  nodes: CubeNodeCatalog;
}

/** Keep Cube save, metadata, and destructive-editor routing out of host adapters. */
export class CubeHostAffordanceController {
  readonly #getRuntime: () => CubeAffordanceRuntime | null;
  readonly #cubeCreation: CubeCreationSave;
  readonly #cubeSave: ExistingCubeSave;
  readonly #confirm: Pick<ConfirmDialog, 'open'>;
  readonly #feedback: AffordanceFeedback | null;
  readonly #prepareGraphClear: () => void;
  readonly #markGraphDirty: () => void;
  readonly #announceGraphCleared: () => void;

  /** Bind existing authoritative application services behind one affordance boundary. */
  constructor(options: {
    getRuntime(): CubeAffordanceRuntime | null;
    cubeCreation: CubeCreationSave;
    cubeSave: ExistingCubeSave;
    confirm: Pick<ConfirmDialog, 'open'>;
    feedback?: AffordanceFeedback | null;
    prepareGraphClear(): void;
    markGraphDirty(): void;
    announceGraphCleared(): void;
  }) {
    this.#getRuntime = options.getRuntime;
    this.#cubeCreation = options.cubeCreation;
    this.#cubeSave = options.cubeSave;
    this.#confirm = options.confirm;
    this.#feedback = options.feedback ?? null;
    this.#prepareGraphClear = options.prepareGraphClear;
    this.#markGraphDirty = options.markGraphDirty;
    this.#announceGraphCleared = options.announceGraphCleared;
  }

  /** Save one selected Cube through draft or persisted-Cube ownership. */
  async saveCube(node: CubeNode): Promise<void> {
    if (isDraftCubeNode(node)) {
      await this.#cubeCreation.saveDraft(
        readInstanceId(node),
        readCubeNodeAuthoringCandidate(node),
      );
      return;
    }
    const cubeId = readIdentityString(node, 'cube_id');
    const outcome = await this.#cubeSave.save({ cubeIds: [cubeId] });
    if (
      outcome.status === 'saved' ||
      outcome.status === 'no_changes' ||
      outcome.status === 'cancelled'
    ) {
      return;
    }
    throw new Error(outcome.message || 'The Cube was not saved.');
  }

  /** Save only at the Cube editor root, retaining current HUD metadata drafts. */
  saveActiveEditor(currentGraph: object | null): boolean {
    const runtime = this.#getRuntime();
    const context = runtime?.contexts.resolveEditor(currentGraph) ?? null;
    return context?.isCubeRoot === true && runtime?.metadataHud?.requestSave() === true;
  }

  /** Expand and focus the existing Cube metadata HUD. */
  focusMetadata(): void {
    this.#getRuntime()?.metadataHud?.focus();
  }

  /** Route description metadata into the active Sugar draft and save owner. */
  setDescription(description: string | null): void {
    const hud = this.#getRuntime()?.metadataHud;
    if (description === null) {
      hud?.focus();
      return;
    }
    if (!hud?.setDescription(description.trim())) {
      this.#feedback?.push?.(
        'info',
        'Cube metadata is read-only',
        'Fork or sync this Cube before changing its description.',
      );
    }
  }

  /** Clear authored implementation nodes while retaining native Cube boundaries and metadata. */
  async clearActiveCube(currentGraph: object | null): Promise<boolean> {
    const runtime = this.#getRuntime();
    const context = runtime?.contexts.resolveEditor(currentGraph) ?? null;
    if (!runtime || !context?.isCubeRoot) return false;
    const confirmed = await this.#confirm.open({
      title: 'Clear Cube implementation?',
      message:
        'This removes every node and connection inside the Cube while keeping its inputs, outputs, and metadata.',
      confirmLabel: 'Clear Cube',
    });
    if (!confirmed) return true;
    this.#prepareGraphClear();
    const graph = requireMutableGraph(context.node.subgraph);
    for (const node of [...graph._nodes]) graph.remove(node);
    runtime.nodes.changed(context.node);
    this.#markGraphDirty();
    this.#announceGraphCleared();
    return true;
  }

  /** Reject a stale or programmatic structural action with Cube-specific guidance. */
  blocked(
    operation:
      | 'convert'
      | 'unpack'
      | 'publish'
      | 'configure-interface'
      | 'search-aliases'
      | 'node-info',
  ): void {
    const details: Record<typeof operation, string> = {
      convert: 'A SugarCube is already a reusable graph unit.',
      unpack: 'Open the Cube to edit its implementation.',
      publish: 'SugarCubes are saved as versioned .cube documents, not Subgraph Blueprints.',
      'configure-interface':
        'SugarCube interfaces are part of Cube authoring and are not configured through Subgraph widget tools.',
      'search-aliases': 'Subgraph Blueprint aliases do not apply to SugarCubes.',
      'node-info': 'Cube details are available from the Cube metadata editor.',
    };
    this.#feedback?.push?.('info', 'SugarCube action unavailable', details[operation]);
  }
}

/** Require one saved Cube identity without weakening save-service invariants. */
function readIdentityString(node: CubeNode, key: string): string {
  const value = requireCubeIdentity(node)[key];
  const normalized = typeof value === 'string' ? value.trim() : '';
  if (!normalized) throw new Error('The Cube is missing its saved identity.');
  return normalized;
}

/** Validate native graph removal before a confirmed clear operation. */
function requireMutableGraph(value: object): MutableCubeGraph {
  const graph = value as Partial<MutableCubeGraph>;
  if (!Array.isArray(graph._nodes) || typeof graph.remove !== 'function') {
    throw new TypeError('The active Cube implementation cannot be cleared safely.');
  }
  return graph as MutableCubeGraph;
}
