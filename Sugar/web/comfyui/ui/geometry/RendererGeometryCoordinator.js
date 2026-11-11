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
/** Coordinate live renderer measurement from immutable cube layout baselines. */
import { applyMeasuredInstanceGeometry } from './ComfyInstanceGeometry.js';
import { applyComfyNodeSize, hasMountedNodePresentation } from './ComfyNodeGeometry.js';
import { collectLiveGeometryInstances } from './LiveCubeGeometryIndex.js';
import { resolveRendererGeometryPolicy } from './RendererGeometryPolicy.js';
const MAX_MOUNT_ATTEMPTS = 20;
const MOUNT_RETRY_DELAY_MS = 50;
const VERIFICATION_DELAYS_MS = [150, 500];
/** Rebuild managed cube presentation from authored layout after host measurement. */
export class RendererGeometryCoordinator {
    adapter;
    scheduler;
    onStabilized;
    pollIntervalMs;
    renderer;
    timerId;
    active;
    generation;
    constructor({ adapter, scheduler, onStabilized, pollIntervalMs = 100, }) {
        this.adapter = adapter;
        this.scheduler = scheduler;
        this.onStabilized = onStabilized ?? null;
        this.pollIntervalMs = pollIntervalMs;
        this.renderer = resolveRendererGeometryPolicy(adapter.getLiteGraph?.(), adapter.getNodeRenderer?.()).renderer;
        this.timerId = null;
        this.active = false;
        this.generation = 0;
    }
    /** Start renderer observation without treating current live rectangles as authored data. */
    setup() {
        if (this.active)
            return;
        this.active = true;
        this.schedulePoll();
    }
    /** Stop renderer observation and invalidate queued measurement passes. */
    dispose() {
        this.active = false;
        this.generation += 1;
        if (this.timerId != null)
            this.scheduler.clearTimeout(this.timerId);
        this.timerId = null;
    }
    /** Stabilize newly imported instances after the active renderer mounts their nodes. */
    scheduleStabilization(_nodes) {
        this.scheduleReflow();
    }
    /** Check renderer state once; exposed for deterministic lifecycle tests. */
    checkRenderer() {
        const nextRenderer = resolveRendererGeometryPolicy(this.adapter.getLiteGraph?.(), this.adapter.getNodeRenderer?.()).renderer;
        if (nextRenderer === this.renderer)
            return;
        this.renderer = nextRenderer;
        this.scheduleReflow();
    }
    schedulePoll() {
        if (!this.active)
            return;
        this.timerId = this.scheduler.timeout(() => {
            this.timerId = null;
            this.checkRenderer();
            this.schedulePoll();
        }, this.pollIntervalMs);
    }
    scheduleReflow() {
        const graph = this.adapter.getGraph?.();
        if (!graph)
            return;
        const generation = ++this.generation;
        const policy = resolveRendererGeometryPolicy(this.adapter.getLiteGraph?.(), this.adapter.getNodeRenderer?.());
        const instances = collectLiveGeometryInstances(graph);
        for (const instance of instances) {
            for (const [identity, authored] of Object.entries(instance.baseline.entries)) {
                const node = instance.nodes.get(identity);
                if (node)
                    applyComfyNodeSize(node, [authored.w, authored.h], policy, this.adapter.getConsole?.());
            }
        }
        this.scheduleMountedReflow(graph, policy.renderer, generation, instances.length, 0);
    }
    scheduleMountedReflow(graph, renderer, generation, expectedInstanceCount, attempt) {
        const inspect = () => {
            if (generation !== this.generation)
                return;
            const instances = collectLiveGeometryInstances(graph);
            const documentRef = this.adapter.getDocument?.() ?? null;
            const graphReady = instances.length >= expectedInstanceCount;
            const rendererReady = renderer !== 'vue' ||
                instances.every(({ nodes }) => [...nodes.values()].every((node) => hasMountedNodePresentation(node, documentRef)));
            if (graphReady && rendererReady) {
                this.reflow(graph, renderer);
                this.scheduleVerification(graph, renderer, generation, 0);
                return;
            }
            if (attempt + 1 < MAX_MOUNT_ATTEMPTS) {
                this.scheduleMountedReflow(graph, renderer, generation, expectedInstanceCount, attempt + 1);
                return;
            }
            this.adapter.getConsole?.()?.warn?.('SugarCubes geometry mount did not settle', {
                renderer,
                expectedInstanceCount,
                mountedInstanceCount: instances.length,
            });
            this.reflow(graph, renderer);
        };
        const scheduled = this.scheduler.timeout(inspect, MOUNT_RETRY_DELAY_MS);
        if (scheduled == null)
            this.afterFrames(2, inspect);
    }
    scheduleVerification(graph, renderer, generation, delayIndex) {
        const delay = VERIFICATION_DELAYS_MS[delayIndex];
        if (delay == null)
            return;
        const verify = () => {
            if (generation !== this.generation)
                return;
            this.reflow(graph, renderer);
            this.scheduleVerification(graph, renderer, generation, delayIndex + 1);
        };
        const scheduled = this.scheduler.timeout(verify, delay);
        if (scheduled == null)
            this.afterFrames(2, verify);
    }
    afterFrames(remaining, callback) {
        if (remaining <= 0) {
            callback();
            return;
        }
        const scheduled = this.scheduler.raf(() => this.afterFrames(remaining - 1, callback));
        if (scheduled == null)
            this.afterFrames(remaining - 1, callback);
    }
    reflow(graph, renderer) {
        let changed = false;
        const instances = collectLiveGeometryInstances(graph);
        for (const instance of instances) {
            changed =
                applyMeasuredInstanceGeometry(instance, {
                    renderer,
                    document: this.adapter.getDocument?.() ?? null,
                }) || changed;
        }
        if (!changed)
            return;
        graph.setDirtyCanvas?.(true, true);
        this.adapter.getCanvas?.()?.setDirty?.(true, true);
        this.adapter.getConsole?.()?.debug?.('SugarCubes renderer geometry stabilized', {
            renderer,
            instanceCount: instances.length,
        });
        this.onStabilized?.(graph);
    }
}
