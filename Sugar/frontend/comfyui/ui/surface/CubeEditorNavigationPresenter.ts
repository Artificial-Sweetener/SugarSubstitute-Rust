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
/** Coordinate Cube editor sessions around Comfy's native graph editor. */

import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import type { CubeNodeCatalog } from '../cube/node/CubeNodeCatalog.js';
import { readInstanceId } from '../cube/node/CubeNodeCatalog.js';
import { isRecord } from '../types/common.js';
import { EMPTY_CUBE_BOUNDARY_VIEWPORT_BOUNDS } from '../cube/geometry/NativeCubeBoundaryLayout.js';
import type { CubeCanvasViewState } from './ComfyCanvasViewStateAdapter.js';
import type { CanvasGraphChangeSource } from './ComfyCanvasGraphChangeAdapter.js';
import type { GraphViewportBounds } from './ComfyCanvasGraphFocusAdapter.js';
import type { CubeEditorMetadataHud } from './CubeEditorMetadataHud.js';
import { CubeEditorContextResolver, type CubeEditorContext } from './CubeEditorContextResolver.js';

const DRAFT_FRAME_ATTEMPTS = 12;

interface CubeEditorCanvas {
  setGraph(graph: object): void;
  openSubgraph?(subgraph: object, fromNode: object): void;
  focusBounds?(bounds: GraphViewportBounds): boolean;
  captureView(): CubeCanvasViewState | null;
  restoreView(state: CubeCanvasViewState): void;
  captureSelection(): readonly unknown[];
  restoreSelection(items: readonly unknown[]): void;
}

export interface CubeEditorNavigationPresenterOptions {
  canvas: CubeEditorCanvas;
  rootGraph: object;
  getCurrentGraph(): object | null;
  nodes: CubeNodeCatalog;
  graphChanges?: CanvasGraphChangeSource;
  contexts?: CubeEditorContextResolver;
  scheduleFrame?(callback: () => void): void;
  metadataHud?: CubeEditorMetadataHud;
}

/** Preserve Cube editing context without replacing Comfy's native graph navigation. */
export class CubeEditorNavigationPresenter {
  readonly #canvas: CubeEditorCanvas;
  readonly #rootGraph: object;
  readonly #getCurrentGraph: () => object | null;
  readonly #nodes: CubeNodeCatalog;
  readonly #contexts: CubeEditorContextResolver;
  readonly #scheduleFrame: (callback: () => void) => void;
  readonly #metadataHud: CubeEditorMetadataHud | null;
  readonly #unsubscribeGraphChanges: () => void;
  #activeCubeInstanceId = '';
  #sessionPath: object[] = [];
  #lastGraph: object | null = null;
  #rootView: CubeCanvasViewState | null = null;
  #rootSelection: readonly unknown[] = [];
  #draftAwaitingFrame: CubeNode | null = null;
  #draftFrameScheduled = false;
  #draftFrameAttempts = 0;
  #framedEmptyCubeGraph: object | null = null;

  /** Bind Cube session tracking to the active native graph editor. */
  constructor(options: CubeEditorNavigationPresenterOptions) {
    this.#canvas = options.canvas;
    this.#rootGraph = options.rootGraph;
    this.#getCurrentGraph = options.getCurrentGraph;
    this.#nodes = options.nodes;
    this.#contexts = options.contexts ?? new CubeEditorContextResolver(options.nodes);
    this.#scheduleFrame = options.scheduleFrame ?? scheduleBrowserFrame;
    this.#metadataHud = options.metadataHud ?? null;
    this.#unsubscribeGraphChanges =
      options.graphChanges?.subscribe(() => this.#sync()) ?? (() => undefined);
    this.#sync();
  }

  /** Release Cube-session state and its navigation observer. */
  dispose(): void {
    this.#unsubscribeGraphChanges();
    this.#activeCubeInstanceId = '';
    this.#sessionPath = [];
    this.#lastGraph = null;
    this.#rootView = null;
    this.#rootSelection = [];
    this.#draftAwaitingFrame = null;
    this.#draftFrameScheduled = false;
    this.#draftFrameAttempts = 0;
    this.#framedEmptyCubeGraph = null;
    this.#metadataHud?.dispose();
  }

  /** Reconcile after an explicit host navigation event. */
  refresh(): void {
    this.#sync();
  }

  /** Open one Cube through Comfy's native editor while retaining Cube context. */
  open(node: CubeNode): void {
    this.prepare(node);
    if (this.#canvas.openSubgraph) {
      this.#canvas.openSubgraph(node.subgraph, node);
    } else {
      this.#canvas.setGraph(node.subgraph);
    }
    this.#sync();
  }

  /** Prepare Cube context before Comfy's own footer action opens the native editor. */
  prepare(node: CubeNode): void {
    this.#rootView = this.#canvas.captureView();
    this.#rootSelection = this.#canvas.captureSelection();
    this.#activeCubeInstanceId = readInstanceId(node);
    this.#sessionPath = [node.subgraph];
    this.#lastGraph = node.subgraph;
    this.#draftAwaitingFrame = shouldFrameEmptyCube(node) ? node : null;
    this.#draftFrameAttempts = 0;
    this.#framedEmptyCubeGraph = null;
    this.#scheduleDraftFrame();
  }

  /** Frame a pristine draft only after Comfy has rendered its native graph transition. */
  #scheduleDraftFrame(): void {
    if (!this.#draftAwaitingFrame || this.#draftFrameScheduled) return;
    this.#draftFrameScheduled = true;
    this.#scheduleFrame(() => {
      this.#draftFrameScheduled = false;
      const node = this.#draftAwaitingFrame;
      const currentGraph = this.#getCurrentGraph();
      if (!node) return;
      if (!currentGraph || !sameGraph(currentGraph, node.subgraph)) {
        if (this.#draftFrameAttempts < DRAFT_FRAME_ATTEMPTS) {
          this.#draftFrameAttempts += 1;
          this.#scheduleDraftFrame();
        }
        return;
      }
      this.#draftAwaitingFrame = null;
      this.#draftFrameAttempts = 0;
      this.#framedEmptyCubeGraph = currentGraph;
      this.#canvas.focusBounds?.(EMPTY_CUBE_BOUNDARY_VIEWPORT_BOUNDS);
    });
  }

  /** Keep Cube context visible while Comfy navigates nested native subgraphs. */
  #sync(): void {
    const currentGraph = this.#getCurrentGraph();
    if (!currentGraph || sameGraph(currentGraph, this.#rootGraph)) {
      const returnedFromCube = this.#activeCubeInstanceId.length > 0;
      const rootView = returnedFromCube ? this.#rootView : null;
      const rootSelection = returnedFromCube ? this.#rootSelection : [];
      this.#activeCubeInstanceId = '';
      this.#sessionPath = [];
      this.#lastGraph = currentGraph;
      this.#rootView = null;
      this.#rootSelection = [];
      this.#draftAwaitingFrame = null;
      this.#draftFrameAttempts = 0;
      this.#framedEmptyCubeGraph = null;
      if (rootView) this.#canvas.restoreView(rootView);
      if (returnedFromCube) this.#canvas.restoreSelection(rootSelection);
    }
    this.#scheduleDraftFrame();
    const activeNode = this.#activeCubeInstanceId
      ? this.#nodes.get(this.#activeCubeInstanceId)
      : null;
    const context = currentGraph
      ? activeNode
        ? {
            node: activeNode,
            path: this.#reconcileSessionPath(activeNode, currentGraph),
            isCubeRoot: sameGraph(activeNode.subgraph, currentGraph),
          }
        : this.#contexts.resolveEditor(currentGraph)
      : null;
    if (context && currentGraph) {
      this.#activeCubeInstanceId = readInstanceId(context.node);
      this.#sessionPath = [...context.path];
      this.#scheduleEmptyCubeFrameFromActiveGraph(context, currentGraph);
      this.#metadataHud?.show(context.node);
    } else {
      this.#metadataHud?.hide();
    }
    this.#lastGraph = currentGraph;
  }

  /** Detect native Cube entry even when Comfy bypasses the footer interaction seam. */
  #scheduleEmptyCubeFrameFromActiveGraph(context: CubeEditorContext, currentGraph: object): void {
    if (
      !shouldFrameEmptyCube(context.node) ||
      (this.#framedEmptyCubeGraph && sameGraph(this.#framedEmptyCubeGraph, currentGraph))
    ) {
      return;
    }
    if (
      !this.#draftAwaitingFrame ||
      !sameGraph(this.#draftAwaitingFrame.subgraph, context.node.subgraph)
    ) {
      this.#draftAwaitingFrame = context.node;
      this.#draftFrameAttempts = 0;
    }
    this.#scheduleDraftFrame();
  }

  /** Preserve the actual graph objects Comfy visited during one Cube edit session. */
  #reconcileSessionPath(node: CubeNode, currentGraph: object): object[] {
    const visitedIndex = this.#sessionPath.findIndex((graph) => sameGraph(graph, currentGraph));
    if (visitedIndex >= 0) {
      const path = this.#sessionPath.slice(0, visitedIndex + 1);
      path[visitedIndex] = currentGraph;
      return path;
    }

    const definitionPath = this.#contexts.resolveEditor(currentGraph)?.path;
    if (definitionPath) return [...definitionPath];

    const previousGraph = this.#lastGraph;
    const sessionTail = this.#sessionPath.at(-1);
    if (previousGraph && sessionTail && sameGraph(previousGraph, sessionTail)) {
      return [...this.#sessionPath, currentGraph];
    }

    return [node.subgraph, currentGraph];
  }
}

/** Limit automatic framing to Cubes with no authored internal content. */
function shouldFrameEmptyCube(node: CubeNode): boolean {
  return node.subgraph._nodes.length === 0;
}

/** Defer visual work until the native graph transition has painted. */
function scheduleBrowserFrame(callback: () => void): void {
  if (typeof requestAnimationFrame === 'function') {
    requestAnimationFrame(() => callback());
    return;
  }
  setTimeout(callback, 0);
}

/** Match restored Comfy subgraphs by their globally unique definition identity. */
function sameGraph(left: object, right: object): boolean {
  if (left === right) return true;
  return readGraphId(left) !== '' && readGraphId(left) === readGraphId(right);
}

/** Read one stable graph identity for render invalidation. */
function readGraphId(graph: object): string {
  return isRecord(graph) && typeof graph.id === 'string' ? graph.id : '';
}
