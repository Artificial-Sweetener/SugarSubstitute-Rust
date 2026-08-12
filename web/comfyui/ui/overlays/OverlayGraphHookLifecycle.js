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
/**
 * Own application, graph, collapse, and pointer hook installation.
 */
import { isRecord } from '../types/common.js';
import { writeCanonicalBounds } from '../graph/CubeBounds.js';
/** Install host callbacks while delegating behavior to focused owners. */
export class OverlayGraphHookLifecycle {
    cleanHooked = false;
    graphHooksWrapped = false;
    adapter;
    scheduler;
    target;
    proximity;
    placement;
    chrome;
    proximityPointerMoves;
    proximityGraphMutations;
    events;
    requestDirtyRefresh;
    containmentService;
    collisionService;
    constructor(options) {
        Object.assign(this, options);
        this.adapter = options.adapter;
        this.scheduler = options.scheduler;
        this.target = options.target;
        this.proximity = options.proximity;
        this.placement = options.placement;
        this.chrome = options.chrome;
        this.proximityPointerMoves = options.proximityPointerMoves;
        this.proximityGraphMutations = options.proximityGraphMutations;
        this.events = options.events;
        this.requestDirtyRefresh = options.requestDirtyRefresh;
        this.containmentService = options.containmentService;
        this.collisionService = options.collisionService;
    }
    ensureCleanHook(attempt = 0) {
        if (this.cleanHooked) {
            return;
        }
        const appRef = this.adapter?.getApp?.() || null;
        const MAX_RETRIES = 60;
        const RETRY_DELAY_MS = 250;
        if (typeof appRef?.clean !== 'function') {
            if (attempt < MAX_RETRIES) {
                this.scheduler?.timeout?.(() => this.ensureCleanHook(attempt + 1), RETRY_DELAY_MS);
            }
            return;
        }
        const manager = this;
        const originalClean = appRef.clean;
        appRef.clean = function sugarcubesPatchedClean(...args) {
            manager.proximity?.resetOverlayState?.();
            manager.placement?.stop?.();
            return originalClean.call(this, ...args);
        };
        this.cleanHooked = true;
    }
    ensureCollapseHook(attempt = 0) {
        const liteGraph = this.adapter?.getLiteGraph?.() || null;
        const nodeProto = liteGraph?.LGraphNode?.prototype ?? null;
        const MAX_RETRIES = 60;
        const RETRY_DELAY_MS = 250;
        if (!nodeProto || typeof nodeProto.collapse !== 'function') {
            if (attempt < MAX_RETRIES) {
                this.scheduler?.timeout?.(() => this.ensureCollapseHook(attempt + 1), RETRY_DELAY_MS);
            }
            return;
        }
        if (!nodeProto.__sugarcubes_collapse_hooked) {
            const originalCollapse = nodeProto.collapse;
            nodeProto.collapse = function sugarcubesPatchedCollapse(...args) {
                const wasCollapsed = isRecord(this.flags) && this.flags.collapsed === true;
                const result = originalCollapse.call(this, ...args);
                const isCollapsed = isRecord(this.flags) && this.flags.collapsed === true;
                if (wasCollapsed !== isCollapsed) {
                    const collapseManager = nodeProto.__sugarcubes_collapse_manager;
                    if (collapseManager?.onNodeCollapseToggled) {
                        collapseManager.onNodeCollapseToggled({
                            node: this,
                            wasCollapsed,
                            isCollapsed,
                        });
                    }
                }
                return result;
            };
            nodeProto.__sugarcubes_collapse_hooked = true;
        }
        nodeProto.__sugarcubes_collapse_manager = this.target;
    }
    ensureGraphHooks(attempt = 0) {
        const appRef = this.adapter?.getApp?.() || null;
        const canvas = appRef?.canvas;
        const MAX_RETRIES = 60;
        const RETRY_DELAY_MS = 250;
        if (!canvas) {
            if (attempt < MAX_RETRIES) {
                this.scheduler?.timeout?.(() => this.ensureGraphHooks(attempt + 1), RETRY_DELAY_MS);
            }
            return;
        }
        if (this.graphHooksWrapped) {
            return;
        }
        this.graphHooksWrapped = true;
        const manager = this.target;
        const lifecycle = this;
        const canvasElement = canvas.canvas ?? null;
        if (canvasElement)
            this.proximityPointerMoves.attach(canvasElement, canvas);
        if (canvasElement && !canvasElement.__sugarcubes_chrome_listener) {
            const handler = (event) => {
                if (lifecycle.chrome?.handleMouseDown?.(event, canvas)) {
                    event.preventDefault();
                    event.stopPropagation();
                    return false;
                }
                return true;
            };
            canvasElement.addEventListener('pointerdown', handler, true);
            canvasElement.addEventListener('mousedown', handler, true);
            canvasElement.__sugarcubes_chrome_listener = handler;
        }
        if (canvasElement && !canvasElement.__sugarcubes_chrome_move_listener) {
            const moveHandler = (event) => {
                if (lifecycle.chrome?.handlePointerMove?.(event, canvas)) {
                    event.preventDefault();
                }
            };
            canvasElement.addEventListener('pointermove', moveHandler, true);
            canvasElement.addEventListener('mousemove', moveHandler, true);
            canvasElement.__sugarcubes_chrome_move_listener = moveHandler;
        }
        const originalAfterChange = canvas.onAfterChange;
        canvas.onAfterChange = function onAfterChange(...args) {
            const managedSnapshot = manager.snapshotManagedGroupBounds(this.graph);
            const preCommit = manager.commitSelectedGroupBounds(this.graph, this);
            const result = originalAfterChange?.call(this, ...args);
            const postCommit = manager.commitSelectedGroupBounds(this.graph, this);
            const committedSet = new Map();
            const recordCommitted = (entry) => {
                const instanceId = entry?.metadata?.instance_id;
                if (!instanceId) {
                    return;
                }
                committedSet.set(String(instanceId), entry);
            };
            recordCommitted(preCommit);
            recordCommitted(postCommit);
            for (const entry of manager.commitManagedGroupBoundsChanges(this.graph, managedSnapshot)) {
                recordCommitted(entry);
            }
            if (committedSet.size &&
                lifecycle.containmentService &&
                lifecycle.collisionService &&
                this.graph) {
                const index = lifecycle.containmentService.buildIndex(this.graph);
                for (const instanceId of committedSet.keys()) {
                    lifecycle.collisionService.resolveCollisions({
                        graph: this.graph,
                        activeInstanceId: instanceId,
                        index,
                    });
                }
            }
            manager.ensureOverlayHook();
            manager.enqueueNodeMove(this.graph, null, this);
            manager.flushNodeMoves(this.graph);
            lifecycle.proximity.schedulePreview({ verbose: true, graph: this.graph });
            manager.scheduleBoundsReconcile(this.graph);
            lifecycle.events?.emit?.('cube:instances:refresh', {
                graph: this.graph,
                reason: 'graph-change',
            });
            lifecycle.requestDirtyRefresh?.({ graph: this.graph, reason: 'graph-change' });
            return result;
        };
        const originalNodeMoved = canvas.onNodeMoved;
        canvas.onNodeMoved = function onNodeMoved(node, ...args) {
            const response = typeof originalNodeMoved === 'function'
                ? originalNodeMoved.call(this, node, ...args)
                : undefined;
            manager.enqueueNodeMove(this.graph, node, this);
            return response;
        };
        const originalProcessMouseDown = canvas.processMouseDown;
        if (typeof originalProcessMouseDown === 'function') {
            canvas.processMouseDown = function processMouseDown(event, ...args) {
                if (lifecycle.chrome?.handleMouseDown?.(event, this)) {
                    manager.clearGroupDragState();
                    return true;
                }
                if (lifecycle.placement.getState().active) {
                    const handled = lifecycle.placement.handlePlacementMouseDown(event, this, null);
                    if (handled) {
                        manager.clearGroupDragState();
                        return true;
                    }
                }
                const result = originalProcessMouseDown.call(this, event, ...args);
                manager.captureGroupDragState(this);
                return result;
            };
        }
        const originalProcessMouseUp = canvas.processMouseUp;
        if (typeof originalProcessMouseUp === 'function') {
            canvas.processMouseUp = function processMouseUp(...args) {
                const committed = manager.commitGroupDrag(this.graph, this) ||
                    manager.commitSelectedGroupBounds(this.graph, this);
                const result = originalProcessMouseUp.call(this, ...args);
                if (committed?.group && committed?.bounds) {
                    writeCanonicalBounds({
                        group: committed.group,
                        metadata: committed.metadata,
                        bounds: committed.bounds,
                    });
                }
                manager.scheduleBoundsReconcile(this.graph);
                return result;
            };
        }
        const graph = canvas.graph;
        this.proximityGraphMutations.attach(graph);
        if (graph && !graph.__sugarcubes_dirty_wrapped) {
            graph.__sugarcubes_dirty_wrapped = true;
            const wrapGraphHook = (key) => {
                const original = graph[key];
                graph[key] = function wrappedGraphHook(...args) {
                    const result = typeof original === 'function' ? original.call(this, ...args) : undefined;
                    lifecycle.requestDirtyRefresh?.({ graph: this, reason: key });
                    return result;
                };
            };
            wrapGraphHook('onNodeAdded');
            wrapGraphHook('onNodeRemoved');
            wrapGraphHook('onConnectionChange');
            wrapGraphHook('onNodeConnectionChange');
        }
        if (this.proximity.isProximityEnabled()) {
            this.proximity.schedulePreview({ immediate: true, verbose: true, graph: canvas.graph });
        }
    }
}
