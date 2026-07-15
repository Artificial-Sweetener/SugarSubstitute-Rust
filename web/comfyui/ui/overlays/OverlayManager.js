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
 * Own the SugarCubes overlay rendering layer in `frontend/comfyui/ui/overlays/OverlayManager.js`.
 */
import { ProximityOverlay } from './ProximityOverlay.js';
import { PlacementOverlay } from './PlacementOverlay.js';
import { CubeChromeOverlay } from './CubeChromeOverlay.js';
import { readGroupBounds } from '../graph/Bounds.js';
import { writeCanonicalBounds } from '../graph/CubeBounds.js';
import { isRecord } from '../types/common.js';
import { createCubeSourceResolver } from './CubeSourceResolver.js';
function readManagedMetadata(group) {
    const sugarcubes = isRecord(group?.properties) ? group.properties.sugarcubes : null;
    if (!isRecord(sugarcubes) || sugarcubes.managed !== true || !sugarcubes.instance_id) {
        return null;
    }
    return sugarcubes;
}
/**
 * Coordinate overlay manager behavior for the SugarCubes UI.
 */
export class OverlayManager {
    adapter;
    events;
    scheduler;
    proximity;
    placement;
    layoutService;
    chrome;
    overlayDrawHooked;
    overlayWatchdog;
    cleanHooked;
    graphHooksWrapped;
    requestDirtyRefresh;
    containmentService;
    collisionService;
    boundsReconciler;
    nodeMoveQueue;
    nodeMoveScheduled;
    reconcileScheduled;
    expandContainmentRevisionByNodeId;
    groupDragState;
    constructor({ adapter = null, events = null, scheduler = null, storage = null, api = null, cubeApi = null, cubeBrowser = null, saveService = null, flavorService = null, toast = null, applyPreparedImport, reportImportOutcome, buildShiftedPlacementPayload, requestDirtyRefresh = null, layoutService = null, containmentService = null, collisionService = null, boundsReconciler = null, } = {}) {
        this.adapter = adapter;
        this.events = events;
        this.scheduler = scheduler;
        this.proximity = new ProximityOverlay({
            adapter,
            events,
            scheduler,
            storage,
            api,
        });
        this.placement = new PlacementOverlay({
            adapter,
            events,
            scheduler,
            cubeApi,
            cubeBrowser,
            toast,
            ...(applyPreparedImport ? { applyPreparedImport } : {}),
            ...(reportImportOutcome ? { reportImportOutcome } : {}),
            ...(buildShiftedPlacementPayload ? { buildShiftedPlacementPayload } : {}),
        });
        this.layoutService = layoutService || null;
        const saveImplementation = saveService?.saveImplementation;
        const saveCubeDefaults = flavorService?.saveCurrentFaceValuesAsCubeDefaults;
        const chromeActions = {
            ...(saveImplementation
                ? {
                    onSaveImplementation: (metadata) => {
                        if (!metadata?.cube_id) {
                            return;
                        }
                        if (isHistoricalCubeMetadata(metadata)) {
                            toast?.push?.('warn', 'Historical version', 'Spawned historical versions cannot overwrite the current cube.');
                            return;
                        }
                        saveImplementation({ cubeIds: [metadata.cube_id] });
                    },
                }
                : {}),
            ...(typeof saveCubeDefaults === 'function'
                ? {
                    onSaveCubeDefaults: (metadata) => {
                        if (isHistoricalCubeMetadata(metadata)) {
                            toast?.push?.('warn', 'Historical version', 'Spawned historical versions cannot overwrite cube defaults.');
                            return null;
                        }
                        return Reflect.apply(saveCubeDefaults, flavorService, [metadata]);
                    },
                }
                : {}),
            onSwapLeft: (metadata) => this.swapLayout(metadata, -1),
            onSwapRight: (metadata) => this.swapLayout(metadata, 1),
            canSwap: (metadata, direction) => this.canSwapDirection(metadata, direction),
        };
        this.chrome = new CubeChromeOverlay({
            adapter,
            actions: chromeActions,
            resolveSource: createCubeSourceResolver(cubeBrowser),
        });
        this.overlayDrawHooked = false;
        this.overlayWatchdog = { timerId: null, attempts: 0 };
        this.cleanHooked = false;
        this.graphHooksWrapped = false;
        this.requestDirtyRefresh = requestDirtyRefresh;
        this.containmentService = containmentService;
        this.collisionService = collisionService;
        this.boundsReconciler = boundsReconciler;
        this.nodeMoveQueue = new Map();
        this.nodeMoveScheduled = false;
        this.reconcileScheduled = false;
        this.expandContainmentRevisionByNodeId = new Map();
        this.groupDragState = null;
    }
    swapLayout(metadata, direction) {
        if (!this.layoutService || !metadata?.instance_id) {
            return;
        }
        const graph = this.adapter?.getApp?.()?.graph || null;
        if (!graph) {
            return;
        }
        const plan = this.resolveSwapPlan(graph, metadata.instance_id, direction);
        if (!plan) {
            return;
        }
        const { order, current, neighbor } = plan;
        const origin = this.resolveLayoutOrigin(order);
        const gaps = this.resolveLayoutGaps(order);
        this.layoutService.swapOrder({
            graph,
            aId: current.instanceId,
            bId: neighbor.instanceId,
            order,
            layout: {
                origin,
                gaps,
                minGap: 24,
            },
        });
        this.proximity?.refreshOverlayState?.({ recompute: true, graph });
        this.proximity?.schedulePreview?.({ immediate: true, verbose: true, graph });
    }
    resolveLayoutOrigin(order) {
        let minX = Infinity;
        let minY = Infinity;
        for (const entry of order || []) {
            const bounds = entry?.bounds;
            if (!bounds) {
                continue;
            }
            minX = Math.min(minX, bounds.x);
            minY = Math.min(minY, bounds.y);
        }
        if (!Number.isFinite(minX) || !Number.isFinite(minY)) {
            return [0, 0];
        }
        return [minX, minY];
    }
    resolveLayoutGaps(order) {
        const MIN_GAP = 24;
        const gaps = [];
        for (let idx = 0; idx < (order?.length || 0) - 1; idx += 1) {
            const current = order[idx];
            const next = order[idx + 1];
            const currentBounds = current?.bounds;
            const nextBounds = next?.bounds;
            if (!currentBounds || !nextBounds) {
                gaps.push(MIN_GAP);
                continue;
            }
            const spacing = nextBounds.x - (currentBounds.x + currentBounds.w);
            const resolved = Number.isFinite(spacing) ? spacing : MIN_GAP;
            gaps.push(Math.max(MIN_GAP, resolved));
        }
        return gaps;
    }
    canSwapEntry(entry) {
        const markers = entry?.markerLookup;
        if (!markers) {
            return false;
        }
        const inputs = Array.isArray(markers.inputs) ? markers.inputs : [];
        const outputs = Array.isArray(markers.outputs) ? markers.outputs : [];
        return inputs.length > 0 && outputs.length > 0;
    }
    canSwapDirection(metadata, direction) {
        if (!metadata?.instance_id) {
            return false;
        }
        const graph = this.adapter?.getApp?.()?.graph || null;
        if (!graph) {
            return false;
        }
        const dir = direction === 'left' || direction === -1 ? -1 : 1;
        const plan = this.resolveSwapPlan(graph, metadata.instance_id, dir);
        return Boolean(plan?.neighbor);
    }
    resolveProximityMatchesForSwap(graph) {
        if (!this.proximity?.settings?.enabled) {
            return [];
        }
        try {
            const matches = Array.isArray(this.proximity?.overlayMatches)
                ? this.proximity.overlayMatches
                : [];
            if (!graph) {
                return matches;
            }
            return matches.filter((match) => match?.outputNode?.graph === graph &&
                match?.inputNode?.graph === graph &&
                match?.outputId != null &&
                match?.inputId != null);
        }
        catch (error) {
            this.adapter
                ?.getConsole?.()
                ?.warn?.('SugarCubes: failed to compute proximity swap matches', error);
            return [];
        }
    }
    resolveSwapPlan(graph, instanceId, direction) {
        const layoutService = this.layoutService;
        if (!layoutService) {
            return null;
        }
        const index = layoutService.buildIndex(graph);
        const proximityMatches = this.resolveProximityMatchesForSwap(graph);
        const order = layoutService.deriveOrder(index, {
            graph,
            anchorInstanceId: instanceId,
            proximityMatches,
        });
        const currentIndex = order.findIndex((entry) => entry?.instanceId === instanceId);
        if (currentIndex < 0) {
            return null;
        }
        const current = order[currentIndex];
        if (!this.canSwapEntry(current)) {
            return null;
        }
        let nextIndex = currentIndex + direction;
        let neighbor = null;
        while (nextIndex >= 0 && nextIndex < order.length) {
            const candidate = order[nextIndex];
            if (this.canSwapEntry(candidate)) {
                neighbor = candidate;
                break;
            }
            nextIndex += direction;
        }
        if (!current?.instanceId || !neighbor?.instanceId) {
            return null;
        }
        return { order: order, current, neighbor };
    }
    /** Return observable chrome state without exposing mutable overlay ownership. */
    getChromeDebugState() {
        return this.chrome.getDebugState();
    }
    setup() {
        this.proximity.installInterceptors();
        this.chrome.setup();
        this.ensureOverlayHook();
        this.startOverlayWatchdog();
        this.ensureGraphHooks();
        this.ensureCollapseHook();
        this.ensureCleanHook();
    }
    dispose() {
        if (this.overlayWatchdog.timerId != null) {
            this.adapter?.getWindow?.()?.clearInterval?.(this.overlayWatchdog.timerId);
            this.overlayWatchdog.timerId = null;
        }
        this.chrome?.dispose?.();
    }
    isOverlayHookActive(canvas) {
        if (!canvas) {
            return false;
        }
        const liteGraph = this.adapter?.getLiteGraph?.() || null;
        const proto = liteGraph?.LGraphCanvas?.prototype ?? null;
        if (proto?.__sugarcubes_overlay_hooked) {
            return true;
        }
        return Boolean(canvas.onDrawForeground?.__sugarcubes_overlay_hooked ||
            canvas.onDrawBackground?.__sugarcubes_overlay_hooked);
    }
    ensureOverlayHook(attempt = 0) {
        const appRef = this.adapter?.getApp?.() || null;
        const canvas = (appRef?.canvas ?? appRef?.graph?.canvas ?? null);
        const MAX_RETRIES = 60;
        const RETRY_DELAY_MS = 250;
        if (!canvas) {
            if (attempt < MAX_RETRIES) {
                this.scheduler?.timeout?.(() => this.ensureOverlayHook(attempt + 1), RETRY_DELAY_MS);
            }
            return;
        }
        if (this.overlayDrawHooked && this.isOverlayHookActive(canvas)) {
            return;
        }
        const liteGraph = this.adapter?.getLiteGraph?.() || null;
        const canvasProto = liteGraph?.LGraphCanvas?.prototype ?? null;
        const manager = this;
        if (canvasProto && typeof canvasProto.drawConnections === 'function') {
            if (!canvasProto.__sugarcubes_proximity_hooked) {
                canvasProto.__sugarcubes_proximity_hooked = true;
                const originalDrawConnections = canvasProto.drawConnections;
                canvasProto.drawConnections = function drawConnections(ctx, ...args) {
                    const result = originalDrawConnections.call(this, ctx, ...args);
                    try {
                        manager.proximity.render(ctx, this);
                    }
                    catch (error) {
                        manager.adapter
                            ?.getConsole?.()
                            ?.error?.('SugarCubes: drawConnections overlay failed', error);
                    }
                    return result;
                };
            }
            this.overlayDrawHooked = true;
        }
        if (canvasProto && typeof canvasProto.drawForeground === 'function') {
            if (!canvasProto.__sugarcubes_overlay_hooked) {
                canvasProto.__sugarcubes_overlay_hooked = true;
                const originalDrawForeground = canvasProto.drawForeground;
                canvasProto.drawForeground = function drawForeground(ctx, ...args) {
                    const result = originalDrawForeground.call(this, ctx, ...args);
                    try {
                        manager.placement.render(ctx, this);
                        manager.chrome.render(ctx, this);
                    }
                    catch (error) {
                        manager.adapter
                            ?.getConsole?.()
                            ?.error?.('SugarCubes: drawForeground overlay failed', error);
                    }
                    return result;
                };
            }
            this.overlayDrawHooked = true;
        }
        if (typeof canvas.onDrawForeground === 'function') {
            const previous = canvas.onDrawForeground;
            const wrappedForeground = function onDrawForeground(ctx, ...args) {
                try {
                    previous.call(this, ctx, ...args);
                }
                catch (error) {
                    manager.adapter
                        ?.getConsole?.()
                        ?.error?.('SugarCubes: onDrawForeground wrapper failed', error);
                }
                manager.placement.render(ctx, this);
                manager.chrome.render(ctx, this);
            };
            wrappedForeground.__sugarcubes_overlay_hooked = true;
            canvas.onDrawForeground = wrappedForeground;
            this.overlayDrawHooked = true;
            if (!this.isOverlayHookActive(canvas) && attempt < MAX_RETRIES) {
                this.scheduler?.timeout?.(() => this.ensureOverlayHook(attempt + 1), RETRY_DELAY_MS);
            }
        }
        const previous = typeof canvas.onDrawBackground === 'function' ? canvas.onDrawBackground : null;
        const wrappedBackground = function onDrawBackground(ctx, ...args) {
            try {
                previous?.call(this, ctx, ...args);
            }
            catch (error) {
                manager.adapter
                    ?.getConsole?.()
                    ?.error?.('SugarCubes: onDrawBackground wrapper failed', error);
            }
            manager.placement.render(ctx, this);
        };
        wrappedBackground.__sugarcubes_overlay_hooked = true;
        canvas.onDrawBackground = wrappedBackground;
        this.overlayDrawHooked = true;
        if (!this.isOverlayHookActive(canvas) && attempt < MAX_RETRIES) {
            this.scheduler?.timeout?.(() => this.ensureOverlayHook(attempt + 1), RETRY_DELAY_MS);
        }
    }
    startOverlayWatchdog() {
        if (this.overlayWatchdog.timerId != null) {
            return;
        }
        this.overlayWatchdog.attempts = 0;
        const windowRef = this.adapter?.getWindow?.() || null;
        if (!windowRef) {
            return;
        }
        this.overlayWatchdog.timerId = windowRef.setInterval(() => {
            this.overlayWatchdog.attempts += 1;
            this.ensureOverlayHook();
            const appRef = this.adapter?.getApp?.() || null;
            const canvas = (appRef?.canvas ?? appRef?.graph?.canvas ?? null);
            if (canvas && this.isOverlayHookActive(canvas)) {
                windowRef.clearInterval(this.overlayWatchdog.timerId ?? undefined);
                this.overlayWatchdog.timerId = null;
                return;
            }
            if (this.overlayWatchdog.attempts >= 20) {
                windowRef.clearInterval(this.overlayWatchdog.timerId ?? undefined);
                this.overlayWatchdog.timerId = null;
            }
        }, 500);
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
                    if (collapseManager instanceof OverlayManager) {
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
        nodeProto.__sugarcubes_collapse_manager = this;
    }
    scheduleAfterFrames(callback, frames) {
        const remaining = Number.isFinite(frames) ? Math.max(0, Math.floor(frames)) : 0;
        if (remaining <= 0) {
            callback();
            return;
        }
        if (typeof this.scheduler?.raf !== 'function') {
            callback();
            return;
        }
        this.scheduler.raf(() => this.scheduleAfterFrames(callback, remaining - 1));
    }
    clearGroupDragState() {
        this.groupDragState = null;
    }
    readManagedGroupEntries(graph) {
        const groups = Array.isArray(graph?._groups) ? graph._groups : graph?.groups || [];
        const entries = [];
        for (const group of groups) {
            const metadata = readManagedMetadata(group);
            if (!metadata) {
                continue;
            }
            const bounds = readGroupBounds(group);
            if (!bounds) {
                continue;
            }
            entries.push({
                instanceId: String(metadata.instance_id),
                group,
                metadata,
                bounds: {
                    x: bounds[0],
                    y: bounds[1],
                    w: bounds[2],
                    h: bounds[3],
                },
            });
        }
        return entries;
    }
    snapshotManagedGroupBounds(graph) {
        const snapshot = new Map();
        for (const entry of this.readManagedGroupEntries(graph)) {
            snapshot.set(entry.instanceId, { ...entry.bounds });
        }
        return snapshot;
    }
    boundsMatch(a, b) {
        if (!isRecord(a)) {
            return false;
        }
        return a.x === b.x && a.y === b.y && a.w === b.w && a.h === b.h;
    }
    commitManagedGroupBoundsChanges(graph, previous = null) {
        const entries = this.readManagedGroupEntries(graph);
        const committed = [];
        for (const entry of entries) {
            const prior = previous?.get?.(entry.instanceId) || null;
            const metadataBounds = entry.metadata?.bounds || null;
            const metadataMatches = this.boundsMatch(metadataBounds, entry.bounds);
            const changedSinceSnapshot = !prior || !this.boundsMatch(prior, entry.bounds);
            if (metadataMatches && !changedSinceSnapshot) {
                continue;
            }
            const canonicalBounds = entry.metadata.bounds
                ? {
                    ...entry.bounds,
                    padding: entry.metadata.bounds.padding,
                    header: entry.metadata.bounds.header,
                }
                : { ...entry.bounds };
            writeCanonicalBounds({
                group: entry.group,
                metadata: entry.metadata,
                bounds: canonicalBounds,
            });
            committed.push({
                group: entry.group,
                metadata: entry.metadata,
                bounds: canonicalBounds,
            });
        }
        return committed;
    }
    resolveManagedCanvasGroup(canvas) {
        const candidates = [];
        const addCandidate = (group) => {
            if (isRecord(group)) {
                candidates.push(group);
            }
        };
        addCandidate(canvas?.selected_group);
        addCandidate(canvas?.resizingGroup);
        if (canvas?.selected_group_resizing && typeof canvas?.selected_group_resizing === 'object') {
            addCandidate(canvas.selected_group_resizing);
        }
        const seen = new Set();
        for (const group of candidates) {
            if (seen.has(group)) {
                continue;
            }
            seen.add(group);
            const metadata = readManagedMetadata(group);
            if (metadata) {
                return { group, metadata };
            }
        }
        return null;
    }
    resolveManagedDragFallback() {
        const state = this.groupDragState;
        if (!state?.group) {
            return null;
        }
        const group = state.group;
        const metadata = readManagedMetadata(group);
        if (!metadata || metadata.instance_id !== state.instanceId) {
            return null;
        }
        const bounds = readGroupBounds(group);
        if (!bounds) {
            return null;
        }
        const changedFromCapture = bounds[0] !== state.bounds.x ||
            bounds[1] !== state.bounds.y ||
            bounds[2] !== state.bounds.w ||
            bounds[3] !== state.bounds.h;
        if (!changedFromCapture) {
            return null;
        }
        return { group, metadata, bounds };
    }
    captureGroupDragState(canvas) {
        const target = this.resolveManagedCanvasGroup(canvas);
        if (!target) {
            this.clearGroupDragState();
            return;
        }
        const { group, metadata } = target;
        const bounds = readGroupBounds(group);
        if (!bounds) {
            this.clearGroupDragState();
            return;
        }
        this.groupDragState = {
            group,
            instanceId: metadata.instance_id,
            bounds: {
                x: bounds[0],
                y: bounds[1],
                w: bounds[2],
                h: bounds[3],
            },
        };
    }
    commitGroupDrag(graph, canvas) {
        const state = this.groupDragState;
        this.clearGroupDragState();
        if (!state?.group) {
            return null;
        }
        const group = state.group;
        const metadata = readManagedMetadata(group);
        if (!metadata || metadata.instance_id !== state.instanceId) {
            return null;
        }
        const bounds = readGroupBounds(group);
        if (!bounds) {
            return null;
        }
        const nextBounds = {
            x: bounds[0],
            y: bounds[1],
            w: bounds[2],
            h: bounds[3],
        };
        const changed = nextBounds.x !== state.bounds.x ||
            nextBounds.y !== state.bounds.y ||
            nextBounds.w !== state.bounds.w ||
            nextBounds.h !== state.bounds.h;
        if (!changed) {
            return null;
        }
        const canonicalBounds = metadata.bounds
            ? {
                ...nextBounds,
                padding: metadata.bounds.padding,
                header: metadata.bounds.header,
            }
            : nextBounds;
        writeCanonicalBounds({
            group,
            metadata,
            bounds: canonicalBounds,
        });
        if (this.containmentService && this.collisionService && graph) {
            const index = this.containmentService.buildIndex(graph);
            if (metadata.instance_id) {
                this.collisionService.resolveCollisions({
                    graph,
                    activeInstanceId: metadata.instance_id,
                    index,
                });
            }
        }
        this.scheduleBoundsReconcile(graph);
        this.requestDirtyRefresh?.({ graph, reason: 'group-move' });
        graph?.setDirtyCanvas?.(true, true);
        const canvasRef = canvas || this.adapter?.getApp?.()?.canvas || null;
        canvasRef?.setDirty?.(true, true);
        return {
            group,
            metadata,
            bounds: canonicalBounds,
        };
    }
    commitSelectedGroupBounds(_graph, canvas) {
        const target = this.resolveManagedCanvasGroup(canvas) || this.resolveManagedDragFallback();
        if (!target) {
            return null;
        }
        const { group, metadata } = target;
        const bounds = target.bounds || readGroupBounds(group);
        if (!bounds || !metadata?.managed || !metadata?.instance_id) {
            return null;
        }
        const canonicalBounds = metadata.bounds
            ? {
                x: bounds[0],
                y: bounds[1],
                w: bounds[2],
                h: bounds[3],
                padding: metadata.bounds.padding,
                header: metadata.bounds.header,
            }
            : {
                x: bounds[0],
                y: bounds[1],
                w: bounds[2],
                h: bounds[3],
            };
        writeCanonicalBounds({ group, metadata, bounds: canonicalBounds });
        if (this.groupDragState?.group === group) {
            this.clearGroupDragState();
        }
        return { group, metadata, bounds: canonicalBounds };
    }
    onNodeCollapseToggled({ node, wasCollapsed, isCollapsed, } = {}) {
        if (!this.containmentService || !node || !wasCollapsed || isCollapsed) {
            return;
        }
        const graph = node?.graph || this.adapter?.getApp?.()?.graph || null;
        if (!graph) {
            return;
        }
        const nodeId = node?.id != null ? String(node.id) : '';
        if (!nodeId) {
            return;
        }
        const revision = (this.expandContainmentRevisionByNodeId.get(nodeId) || 0) + 1;
        this.expandContainmentRevisionByNodeId.set(nodeId, revision);
        this.scheduleAfterFrames(() => {
            if (this.expandContainmentRevisionByNodeId.get(nodeId) !== revision) {
                return;
            }
            if ((isRecord(node.flags) && node.flags.collapsed === true) ||
                !this.isMovableNodeCandidate(node)) {
                return;
            }
            this.enqueueNodes(graph, [node], { schedule: false });
            this.flushNodeMoves(graph);
            this.scheduleBoundsReconcile(graph);
            this.requestDirtyRefresh?.({ graph, reason: 'node-expand' });
            graph?.setDirtyCanvas?.(true, true);
        }, 2);
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
        const manager = this;
        const canvasElement = canvas.canvas ?? null;
        if (canvasElement && !canvasElement.__sugarcubes_chrome_listener) {
            const handler = (event) => {
                if (manager.chrome?.handleMouseDown?.(event, canvas)) {
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
                if (manager.chrome?.handlePointerMove?.(event, canvas)) {
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
                manager.containmentService &&
                manager.collisionService &&
                this.graph) {
                const index = manager.containmentService.buildIndex(this.graph);
                for (const instanceId of committedSet.keys()) {
                    manager.collisionService.resolveCollisions({
                        graph: this.graph,
                        activeInstanceId: instanceId,
                        index,
                    });
                }
            }
            manager.ensureOverlayHook();
            manager.enqueueNodeMove(this.graph, null, this);
            manager.flushNodeMoves(this.graph);
            manager.proximity.schedulePreview({ verbose: true, graph: this.graph });
            manager.scheduleBoundsReconcile(this.graph);
            manager.events?.emit?.('cube:instances:refresh', {
                graph: this.graph,
                reason: 'graph-change',
            });
            manager.requestDirtyRefresh?.({ graph: this.graph, reason: 'graph-change' });
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
        const originalProcessMouseMove = canvas.processMouseMove;
        if (typeof originalProcessMouseMove === 'function') {
            canvas.processMouseMove = function processMouseMove(...args) {
                const res = originalProcessMouseMove.call(this, ...args);
                if (manager.proximity.isOverlayEnabled()) {
                    manager.proximity.schedulePreview({ immediate: true, verbose: true, graph: this.graph });
                }
                return res;
            };
        }
        const originalProcessMouseDown = canvas.processMouseDown;
        if (typeof originalProcessMouseDown === 'function') {
            canvas.processMouseDown = function processMouseDown(event, ...args) {
                if (manager.chrome?.handleMouseDown?.(event, this)) {
                    manager.clearGroupDragState();
                    return true;
                }
                if (manager.placement.getState().active) {
                    const handled = manager.placement.handlePlacementMouseDown(event, this, null);
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
        if (graph && !graph.__sugarcubes_dirty_wrapped) {
            graph.__sugarcubes_dirty_wrapped = true;
            const wrapGraphHook = (key) => {
                const original = graph[key];
                if (typeof original !== 'function') {
                    return;
                }
                graph[key] = function wrappedGraphHook(...args) {
                    const result = original.call(this, ...args);
                    const changedNode = isRecord(args[0]) ? args[0] : null;
                    if (manager.proximity.isOverlayEnabled() &&
                        key === 'onNodeConnectionChange' &&
                        (changedNode?.type === 'SugarCubes.CubeInput' ||
                            changedNode?.type === 'SugarCubes.CubeOutput')) {
                        manager.proximity.schedulePreview({ immediate: true, graph: this });
                    }
                    manager.requestDirtyRefresh?.({ graph: this, reason: key });
                    return result;
                };
            };
            wrapGraphHook('onNodeAdded');
            wrapGraphHook('onNodeRemoved');
            wrapGraphHook('onNodeConnectionChange');
        }
        if (this.proximity.isOverlayEnabled()) {
            this.proximity.schedulePreview({ immediate: true, verbose: true, graph: canvas.graph });
        }
        const originalBackground = canvas.onDrawBackground;
        canvas.onDrawBackground = function onDrawBackground(ctx, ...args) {
            manager.proximity.ensurePreview(this.graph);
            return originalBackground?.call(this, ctx, ...args);
        };
    }
    isMovableNodeCandidate(node) {
        if (!isRecord(node) || node.id == null) {
            return false;
        }
        const pos = node.pos;
        const size = node.size;
        const hasPos = (Array.isArray(pos) || ArrayBuffer.isView(pos)) &&
            Number(pos.length) >= 2;
        const hasSize = (Array.isArray(size) || ArrayBuffer.isView(size)) &&
            Number(size.length) >= 2;
        return hasPos && hasSize;
    }
    collectMovedNodes(node, canvas) {
        const movedById = new Map();
        const addNode = (candidate) => {
            if (!this.isMovableNodeCandidate(candidate)) {
                return;
            }
            const candidateId = String(candidate.id);
            if (!movedById.has(candidateId)) {
                movedById.set(candidateId, candidate);
            }
        };
        const selectedItems = canvas?.selectedItems;
        if (selectedItems?.values && typeof selectedItems.values === 'function') {
            for (const item of selectedItems.values()) {
                addNode(item);
            }
        }
        const legacySelected = canvas?.selected_nodes;
        if (legacySelected && typeof legacySelected === 'object') {
            for (const item of Object.values(legacySelected)) {
                addNode(item);
            }
        }
        if (movedById.size === 0) {
            addNode(node);
        }
        return Array.from(movedById.values());
    }
    enqueueNodeMove(graph, node, canvas) {
        if (!this.containmentService) {
            return;
        }
        const movedNodes = this.collectMovedNodes(node, canvas);
        this.enqueueNodes(graph, movedNodes);
    }
    enqueueNodes(graph, movedNodes, { schedule = true } = {}) {
        if (!this.containmentService || !Array.isArray(movedNodes) || movedNodes.length === 0) {
            return;
        }
        for (const movedNode of movedNodes) {
            const nodeId = String(movedNode.id);
            const entry = this.nodeMoveQueue.get(nodeId) || [];
            entry.push(movedNode);
            this.nodeMoveQueue.set(nodeId, entry);
        }
        if (!schedule || this.nodeMoveScheduled) {
            return;
        }
        this.nodeMoveScheduled = true;
        this.scheduler?.raf?.(() => {
            this.nodeMoveScheduled = false;
            this.flushNodeMoves(graph);
        });
    }
    flushNodeMoves(graph) {
        if (!this.containmentService || !this.collisionService) {
            this.nodeMoveQueue.clear();
            return;
        }
        const nodes = [];
        for (const entries of this.nodeMoveQueue.values()) {
            nodes.push(...entries);
        }
        this.nodeMoveQueue.clear();
        if (!nodes.length) {
            return;
        }
        const index = this.containmentService.buildIndex(graph);
        const { instances } = this.containmentService.enforceForNodes({ graph, nodes, index });
        if (!instances || instances.size === 0) {
            return;
        }
        for (const instanceId of instances) {
            this.collisionService.resolveCollisions({ graph, activeInstanceId: instanceId, index });
        }
    }
    scheduleBoundsReconcile(graph) {
        const boundsReconciler = this.boundsReconciler;
        if (!boundsReconciler || this.reconcileScheduled) {
            return;
        }
        this.reconcileScheduled = true;
        this.scheduler?.raf?.(() => {
            this.reconcileScheduled = false;
            const { changed, index } = boundsReconciler.reconcileAll({ graph });
            if (!changed || changed.size === 0 || !this.collisionService) {
                return;
            }
            for (const instanceId of changed) {
                this.collisionService.resolveCollisions({ graph, activeInstanceId: instanceId, index });
            }
        });
    }
}
function isHistoricalCubeMetadata(metadata) {
    const revisionRef = typeof metadata?.cube_revision_ref === 'string' ? metadata.cube_revision_ref.trim() : '';
    return Boolean(revisionRef && revisionRef !== 'WORKTREE');
}
