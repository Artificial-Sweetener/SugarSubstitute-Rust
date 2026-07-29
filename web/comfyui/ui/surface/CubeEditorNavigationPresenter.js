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
import { isRecord } from '../types/common.js';
import { readInstanceId } from '../cube/node/CubeNodeCatalog.js';
import { EMPTY_CUBE_BOUNDARY_VIEWPORT_BOUNDS } from '../cube/geometry/NativeCubeBoundaryLayout.js';
const DRAFT_FRAME_ATTEMPTS = 12;
/** Preserve Cube editing context without replacing Comfy's native graph navigation. */
export class CubeEditorNavigationPresenter {
    #canvas;
    #rootGraph;
    #getCurrentGraph;
    #nodes;
    #scheduleFrame;
    #metadataHud;
    #unsubscribeGraphChanges;
    #activeCubeInstanceId = '';
    #sessionPath = [];
    #lastGraph = null;
    #rootView = null;
    #rootSelection = [];
    #draftAwaitingFrame = null;
    #draftFrameScheduled = false;
    #draftFrameAttempts = 0;
    #framedEmptyCubeGraph = null;
    /** Bind Cube session tracking to the active native graph editor. */
    constructor(options) {
        this.#canvas = options.canvas;
        this.#rootGraph = options.rootGraph;
        this.#getCurrentGraph = options.getCurrentGraph;
        this.#nodes = options.nodes;
        this.#scheduleFrame = options.scheduleFrame ?? scheduleBrowserFrame;
        this.#metadataHud = options.metadataHud ?? null;
        this.#unsubscribeGraphChanges =
            options.graphChanges?.subscribe(() => this.#sync()) ?? (() => undefined);
        this.#sync();
    }
    /** Release Cube-session state and its navigation observer. */
    dispose() {
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
    refresh() {
        this.#sync();
    }
    /** Open one Cube through Comfy's native editor while retaining Cube context. */
    open(node) {
        this.prepare(node);
        if (this.#canvas.openSubgraph) {
            this.#canvas.openSubgraph(node.subgraph, node);
        }
        else {
            this.#canvas.setGraph(node.subgraph);
        }
        this.#sync();
    }
    /** Prepare Cube context before Comfy's own footer action opens the native editor. */
    prepare(node) {
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
    #scheduleDraftFrame() {
        if (!this.#draftAwaitingFrame || this.#draftFrameScheduled)
            return;
        this.#draftFrameScheduled = true;
        this.#scheduleFrame(() => {
            this.#draftFrameScheduled = false;
            const node = this.#draftAwaitingFrame;
            const currentGraph = this.#getCurrentGraph();
            if (!node)
                return;
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
    #sync() {
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
            if (rootView)
                this.#canvas.restoreView(rootView);
            if (returnedFromCube)
                this.#canvas.restoreSelection(rootSelection);
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
                }
                : findEditorContext(this.#nodes.list(), currentGraph)
            : null;
        if (context && currentGraph) {
            this.#activeCubeInstanceId = readInstanceId(context.node);
            this.#sessionPath = context.path;
            this.#scheduleEmptyCubeFrameFromActiveGraph(context, currentGraph);
            this.#metadataHud?.show(context.node);
        }
        else {
            this.#metadataHud?.hide();
        }
        this.#lastGraph = currentGraph;
    }
    /** Detect native Cube entry even when Comfy bypasses the footer interaction seam. */
    #scheduleEmptyCubeFrameFromActiveGraph(context, currentGraph) {
        if (!shouldFrameEmptyCube(context.node) ||
            (this.#framedEmptyCubeGraph && sameGraph(this.#framedEmptyCubeGraph, currentGraph))) {
            return;
        }
        if (!this.#draftAwaitingFrame ||
            !sameGraph(this.#draftAwaitingFrame.subgraph, context.node.subgraph)) {
            this.#draftAwaitingFrame = context.node;
            this.#draftFrameAttempts = 0;
        }
        this.#scheduleDraftFrame();
    }
    /** Preserve the actual graph objects Comfy visited during one Cube edit session. */
    #reconcileSessionPath(node, currentGraph) {
        const visitedIndex = this.#sessionPath.findIndex((graph) => sameGraph(graph, currentGraph));
        if (visitedIndex >= 0) {
            const path = this.#sessionPath.slice(0, visitedIndex + 1);
            path[visitedIndex] = currentGraph;
            return path;
        }
        const definitionPath = findGraphPath(node.subgraph, currentGraph);
        if (definitionPath)
            return definitionPath;
        const previousGraph = this.#lastGraph;
        const sessionTail = this.#sessionPath.at(-1);
        if (previousGraph && sessionTail && sameGraph(previousGraph, sessionTail)) {
            return [...this.#sessionPath, currentGraph];
        }
        return [node.subgraph, currentGraph];
    }
}
/** Limit automatic framing to Cubes with no authored internal content. */
function shouldFrameEmptyCube(node) {
    return node.subgraph._nodes.length === 0;
}
/** Defer visual work until the native graph transition has painted. */
function scheduleBrowserFrame(callback) {
    if (typeof requestAnimationFrame === 'function') {
        requestAnimationFrame(() => callback());
        return;
    }
    setTimeout(callback, 0);
}
/** Find a current native graph within one Cube definition hierarchy. */
function findEditorContext(nodes, currentGraph) {
    for (const node of nodes) {
        const path = findGraphPath(node.subgraph, currentGraph);
        if (path)
            return { node, path };
    }
    return null;
}
/** Traverse actual nested SubgraphNodes without projecting them onto the root. */
function findGraphPath(graph, target, visited = new Set()) {
    if (sameGraph(graph, target))
        return [graph];
    if (visited.has(graph))
        return null;
    visited.add(graph);
    if (!isRecord(graph) || !Array.isArray(graph._nodes))
        return null;
    for (const node of graph._nodes) {
        if (!isRecord(node) || !isRecord(node.subgraph))
            continue;
        const isSubgraph = typeof node.isSubgraphNode === 'function' ? node.isSubgraphNode.call(node) === true : false;
        if (!isSubgraph)
            continue;
        const nestedPath = findGraphPath(node.subgraph, target, visited);
        if (nestedPath)
            return [graph, ...nestedPath];
    }
    return null;
}
/** Match restored Comfy subgraphs by their globally unique definition identity. */
function sameGraph(left, right) {
    if (left === right)
        return true;
    const leftId = readGraphId(left);
    if (leftId && leftId === readGraphId(right))
        return true;
    const leftName = readGraphName(left);
    return leftName !== 'Subgraph' && leftName === readGraphName(right);
}
/** Read one stable graph identity for render invalidation. */
function readGraphId(graph) {
    return isRecord(graph) && typeof graph.id === 'string' ? graph.id : '';
}
/** Read one native subgraph name for the Cube editor trail. */
function readGraphName(graph) {
    return isRecord(graph) && typeof graph.name === 'string' && graph.name.trim()
        ? graph.name.trim()
        : 'Subgraph';
}
