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
/** Compose focused Cube affordance adapters around one graph-bound runtime. */

import { isCubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import type { ComfyCubeRuntime } from '../cube/ComfyCubeRuntime.js';
import { isRecord } from '../types/common.js';
import { CubeAffordancePolicy } from './CubeAffordancePolicy.js';
import type { CubeHostAffordanceController } from './CubeHostAffordanceController.js';
import { ComfyCubeCommandAdapter } from './ComfyCubeCommandAdapter.js';
import { ComfyCubeEditorChromeAdapter } from './ComfyCubeEditorChromeAdapter.js';
import { ComfyCubeNativeMenuAdapter } from './ComfyCubeNativeMenuAdapter.js';
import { ComfyCubeSaveButtonPresenter } from './ComfyCubeSaveButtonPresenter.js';
import { ComfyCubeSelectionSurfaceAdapter } from './ComfyCubeSelectionSurfaceAdapter.js';
import { ComfyCubeWorkflowActionsMenuAdapter } from './ComfyCubeWorkflowActionsMenuAdapter.js';
import { CubeMissingNodeLabelAdapter } from './CubeMissingNodeLabelAdapter.js';
import { CubeInstanceRenameGuard } from './CubeInstanceRenameGuard.js';
import { CubeNodeTooltipAdapter } from './CubeNodeTooltipAdapter.js';
import { PrimeVueTooltipPresentationAdapter } from './PrimeVueTooltipPresentationAdapter.js';

interface AffordanceCanvas {
  graph?: object;
  subgraph?: object;
  selectedItems: Set<unknown>;
  getNodeMenuOptions(node: unknown): unknown[];
  getCanvasMenuOptions(): unknown[];
}

/** Describe one extension-provided LiteGraph menu item. */
export interface CubeAffordanceMenuItem {
  content: string;
  callback: () => unknown;
}

/** Keep lifecycle composition separate from each renderer or action responsibility. */
export class CubeAffordanceHostIntegration {
  readonly #document: Document;
  readonly #canvas: AffordanceCanvas;
  readonly #controller: CubeHostAffordanceController;
  readonly #logger: Pick<Console, 'error' | 'warn'>;
  readonly #policy = new CubeAffordancePolicy();
  #runtime: ComfyCubeRuntime | null = null;
  #nativeMenus: ComfyCubeNativeMenuAdapter | null = null;
  #commands: ComfyCubeCommandAdapter | null = null;
  #selectionSurface: ComfyCubeSelectionSurfaceAdapter | null = null;
  #editorChrome: ComfyCubeEditorChromeAdapter | null = null;
  #workflowActions: ComfyCubeWorkflowActionsMenuAdapter | null = null;
  #missingNodes: CubeMissingNodeLabelAdapter | null = null;
  #instanceRename: CubeInstanceRenameGuard | null = null;
  #nodeTooltips: CubeNodeTooltipAdapter | null = null;

  /** Bind stable host surfaces; graph-specific collaborators arrive through attach. */
  constructor(options: {
    document: Document;
    canvas: unknown;
    controller: CubeHostAffordanceController;
    logger: Pick<Console, 'error' | 'warn'>;
  }) {
    this.#document = options.document;
    this.#canvas = requireAffordanceCanvas(options.canvas);
    this.#controller = options.controller;
    this.#logger = options.logger;
  }

  /** Rebind adapters after Comfy replaces the root workflow graph. */
  attach(runtime: ComfyCubeRuntime): void {
    if (this.#runtime === runtime) return;
    this.#disposeAdapters();
    this.#runtime = runtime;
    this.#missingNodes = new CubeMissingNodeLabelAdapter(runtime.nodes);
    try {
      this.#nativeMenus = new ComfyCubeNativeMenuAdapter(this.#canvas);
      this.#nativeMenus.install();
      this.#commands = new ComfyCubeCommandAdapter({
        document: this.#document,
        canvas: this.#canvas,
        contexts: runtime.contexts,
        policy: this.#policy,
        controller: this.#controller,
      });
      this.#commands.install();
    } catch (error: unknown) {
      this.#logger.error('SugarCubes: Comfy command/menu compatibility contract failed.', error);
      this.#commands?.dispose();
      this.#commands = null;
    }
    const tooltips = new PrimeVueTooltipPresentationAdapter({
      document: this.#document,
      logger: this.#logger,
    });
    this.#selectionSurface = new ComfyCubeSelectionSurfaceAdapter({
      document: this.#document,
      canvas: this.#canvas,
      contexts: runtime.contexts,
      saveButton: new ComfyCubeSaveButtonPresenter({
        document: this.#document,
        tooltips,
      }),
    });
    this.#selectionSurface.install();
    this.#instanceRename = new CubeInstanceRenameGuard({
      document: this.#document,
      canvas: this.#canvas,
      nodes: runtime.nodes,
      contexts: runtime.contexts,
    });
    this.#instanceRename.install();
    this.#nodeTooltips = new CubeNodeTooltipAdapter({
      document: this.#document,
      nodes: runtime.nodes,
    });
    this.#nodeTooltips.install();
    this.#editorChrome = new ComfyCubeEditorChromeAdapter({
      document: this.#document,
      canvas: this.#canvas,
      contexts: runtime.contexts,
      policy: this.#policy,
      controller: this.#controller,
    });
    this.#editorChrome.install();
    this.#workflowActions = new ComfyCubeWorkflowActionsMenuAdapter({
      document: this.#document,
      canvas: this.#canvas,
      contexts: runtime.contexts,
      policy: this.#policy,
    });
    this.#workflowActions.install();
  }

  /** Supply Cube replacements through Comfy's supported additive node-menu hook. */
  getNodeMenuItems(node: unknown): CubeAffordanceMenuItem[] {
    if (!isCubeNode(node)) return [];
    return [
      {
        content: 'Save Cube',
        callback: () => this.#controller.saveCube(node),
      },
    ];
  }

  /** Adapt missing-node hints before Comfy transfers them into warning stores. */
  adaptMissingNodes(missingNodes: readonly unknown[]): number {
    return this.#missingNodes?.adapt(missingNodes) ?? 0;
  }

  /** Reconcile Vue surfaces after selection changes not accompanied by a DOM mount. */
  refresh(): void {
    this.#selectionSurface?.refresh();
    this.#editorChrome?.refresh();
    this.#workflowActions?.refresh();
  }

  /** Release all host mutations owned by this extension instance. */
  dispose(): void {
    this.#disposeAdapters();
    this.#runtime = null;
  }

  /** Dispose in reverse dependency order before attaching a replacement runtime. */
  #disposeAdapters(): void {
    this.#workflowActions?.dispose();
    this.#editorChrome?.dispose();
    this.#nodeTooltips?.dispose();
    this.#instanceRename?.dispose();
    this.#selectionSurface?.dispose();
    this.#commands?.dispose();
    this.#nativeMenus?.dispose();
    this.#editorChrome = null;
    this.#workflowActions = null;
    this.#selectionSurface = null;
    this.#instanceRename = null;
    this.#nodeTooltips = null;
    this.#commands = null;
    this.#nativeMenus = null;
    this.#missingNodes = null;
  }
}

/** Validate the smallest stable canvas surface shared by both Comfy renderers. */
function requireAffordanceCanvas(value: unknown): AffordanceCanvas {
  if (
    !isRecord(value) ||
    !(value.selectedItems instanceof Set) ||
    typeof value.getNodeMenuOptions !== 'function' ||
    typeof value.getCanvasMenuOptions !== 'function'
  ) {
    throw new TypeError('Comfy Cube affordance canvas integration is unavailable.');
  }
  return value as unknown as AffordanceCanvas;
}
