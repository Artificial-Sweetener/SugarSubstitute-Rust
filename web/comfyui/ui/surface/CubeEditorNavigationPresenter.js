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
/** Present distinct Cube editor navigation around Comfy's native graph editor. */
import { isRecord } from '../types/common.js';
import { readInstanceId } from '../cube/node/CubeNodeCatalog.js';
/** Own the Cube-specific editor trail without replacing Comfy's graph editor. */
export class CubeEditorNavigationPresenter {
    #document;
    #canvas;
    #rootGraph;
    #getCurrentGraph;
    #nodes;
    #logger;
    #unsubscribeGraphChanges;
    #bar = null;
    #signature = '';
    #activeCubeInstanceId = '';
    #sessionPath = [];
    #lastGraph = null;
    #rootView = null;
    #rootSelection = [];
    /** Bind a small Cube navigation bar to the active native graph editor. */
    constructor(options) {
        this.#document = options.document;
        this.#canvas = options.canvas;
        this.#rootGraph = options.rootGraph;
        this.#getCurrentGraph = options.getCurrentGraph;
        this.#nodes = options.nodes;
        this.#logger = options.logger ?? null;
        this.#unsubscribeGraphChanges =
            options.graphChanges?.subscribe(() => this.#sync()) ?? (() => undefined);
        this.#sync();
    }
    /** Release the editor bar and its navigation observer. */
    dispose() {
        this.#unsubscribeGraphChanges();
        this.#bar?.remove();
        this.#bar = null;
        this.#signature = '';
        this.#activeCubeInstanceId = '';
        this.#sessionPath = [];
        this.#lastGraph = null;
        this.#rootView = null;
        this.#rootSelection = [];
    }
    /** Reconcile after an explicit host navigation event. */
    refresh() {
        this.#sync();
    }
    /** Open one Cube through Comfy's native editor while retaining Cube context. */
    open(node) {
        this.#rootView = this.#canvas.captureView();
        this.#rootSelection = this.#canvas.captureSelection();
        this.#activeCubeInstanceId = readInstanceId(node);
        this.#sessionPath = [node.subgraph];
        this.#lastGraph = node.subgraph;
        this.#canvas.setGraph(node.subgraph);
        this.#sync();
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
            if (rootView)
                this.#canvas.restoreView(rootView);
            if (returnedFromCube)
                this.#canvas.restoreSelection(rootSelection);
        }
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
        if (context) {
            this.#activeCubeInstanceId = readInstanceId(context.node);
            this.#sessionPath = context.path;
        }
        this.#lastGraph = currentGraph;
        const nativeNavigation = this.#document.querySelector('[aria-label="Graph navigation"]');
        const nativeNavigationHost = nativeNavigation?.parentElement ?? null;
        const mountParent = nativeNavigationHost?.parentElement ?? null;
        if (!context || !nativeNavigationHost || !mountParent) {
            this.#bar?.remove();
            this.#bar = null;
            this.#signature = '';
            return;
        }
        if (!this.#bar) {
            this.#bar = this.#document.createElement('div');
            this.#bar.className = 'sugarcubes-cube-editor-navigation';
        }
        if (this.#bar.parentElement !== mountParent) {
            mountParent.insertBefore(this.#bar, nativeNavigationHost);
        }
        const signature = `${readInstanceId(context.node)}|${context.path.map(readGraphId).join('|')}`;
        if (signature === this.#signature)
            return;
        this.#signature = signature;
        this.#render(context);
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
    /** Render one explicit Cube trail with native-graph back and exit actions. */
    #render(context) {
        if (!this.#bar)
            return;
        const instanceId = readInstanceId(context.node);
        this.#bar.dataset.cubeNodeId = String(context.node.id);
        this.#bar.dataset.cubeInstanceId = instanceId;
        this.#bar.dataset.cubeGraphPath = context.path
            .map((graph) => `${readGraphId(graph)}:${readGraphName(graph)}`)
            .join('|');
        this.#bar.dataset.cubeRootGraph = `${readGraphId(this.#rootGraph)}:${readGraphName(this.#rootGraph)}`;
        const label = this.#document.createElement('span');
        label.className = 'sugarcubes-cube-editor-navigation__label';
        label.textContent = 'Cube Editor';
        const trail = this.#document.createElement('strong');
        trail.className = 'sugarcubes-cube-editor-navigation__trail';
        trail.textContent = [
            context.node.title?.trim() || context.node.subgraph.name,
            ...context.path.slice(1).map(readGraphName),
        ].join(' / ');
        const actions = this.#document.createElement('span');
        actions.className = 'sugarcubes-cube-editor-navigation__actions';
        if (context.path.length > 1) {
            actions.append(createAction(this.#document, 'Back', () => {
                const parent = context.path.at(-2);
                if (!parent)
                    return;
                this.#logger?.debug('SugarCubes navigating to a parent graph inside the Cube editor.', {
                    instanceId,
                    path: context.path.map(readGraphId),
                    targetGraphId: readGraphId(parent),
                });
                this.#canvas.setGraph(parent);
                queueMicrotask(() => {
                    const current = this.#getCurrentGraph();
                    this.#logger?.debug('SugarCubes completed a parent Cube graph transition.', {
                        instanceId,
                        currentGraphId: current ? readGraphId(current) : '',
                        currentGraphName: current ? readGraphName(current) : '',
                    });
                });
            }));
        }
        actions.append(createAction(this.#document, 'Exit Cube', () => this.#exitToRoot()));
        this.#bar.replaceChildren(label, trail, actions);
    }
    /** Return to the root graph with the exact surface viewport from entry. */
    #exitToRoot() {
        const rootView = this.#rootView;
        const rootSelection = this.#rootSelection;
        this.#canvas.setGraph(this.#rootGraph);
        if (rootView)
            this.#canvas.restoreView(rootView);
        this.#canvas.restoreSelection(rootSelection);
    }
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
/** Create one accessible editor action without dynamic markup. */
function createAction(documentRef, label, action) {
    const button = documentRef.createElement('button');
    button.type = 'button';
    button.textContent = label;
    button.addEventListener('click', (event) => {
        event.preventDefault();
        event.stopPropagation();
        action();
    });
    return button;
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
