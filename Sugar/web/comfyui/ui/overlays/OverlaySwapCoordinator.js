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
 * Own Cube neighbor selection and horizontal swap commands.
 */
/** Coordinate graph-aware Cube swaps without owning host hooks. */
export class OverlaySwapCoordinator {
    adapter;
    layoutService;
    proximity;
    constructor(adapter, layoutService, proximity) {
        this.adapter = adapter;
        this.layoutService = layoutService;
        this.proximity = proximity;
    }
    swapLayout(metadata, direction) {
        if (!this.layoutService || !metadata?.instance_id)
            return;
        const graph = this.adapter?.getApp?.()?.graph || null;
        if (!graph)
            return;
        const plan = this.resolveSwapPlan(graph, metadata.instance_id, direction);
        if (!plan)
            return;
        const { order, current, neighbor } = plan;
        this.layoutService.swapOrder({
            graph,
            aId: current.instanceId,
            bId: neighbor.instanceId,
            order,
            layout: {
                origin: this.resolveLayoutOrigin(order),
                gaps: this.resolveLayoutGaps(order),
                minGap: 24,
            },
        });
        this.proximity.refreshOverlayState?.({ recompute: true, graph });
        this.proximity.schedulePreview?.({ immediate: true, verbose: true, graph });
    }
    resolveLayoutOrigin(order) {
        let minX = Infinity;
        let minY = Infinity;
        for (const entry of order || []) {
            if (!entry?.bounds)
                continue;
            minX = Math.min(minX, entry.bounds.x);
            minY = Math.min(minY, entry.bounds.y);
        }
        return Number.isFinite(minX) && Number.isFinite(minY) ? [minX, minY] : [0, 0];
    }
    resolveLayoutGaps(order) {
        const minimumGap = 24;
        const gaps = [];
        for (let index = 0; index < (order?.length || 0) - 1; index += 1) {
            const current = order[index]?.bounds;
            const next = order[index + 1]?.bounds;
            if (!current || !next) {
                gaps.push(minimumGap);
                continue;
            }
            const spacing = next.x - (current.x + current.w);
            gaps.push(Math.max(minimumGap, Number.isFinite(spacing) ? spacing : minimumGap));
        }
        return gaps;
    }
    canSwapEntry(entry) {
        const inputs = Array.isArray(entry?.markerLookup?.inputs) ? entry.markerLookup.inputs : [];
        const outputs = Array.isArray(entry?.markerLookup?.outputs) ? entry.markerLookup.outputs : [];
        return inputs.length > 0 && outputs.length > 0;
    }
    canSwapDirection(metadata, direction) {
        if (!metadata?.instance_id)
            return false;
        const graph = this.adapter?.getApp?.()?.graph || null;
        if (!graph)
            return false;
        const step = direction === 'left' || direction === -1 ? -1 : 1;
        return Boolean(this.resolveSwapPlan(graph, metadata.instance_id, step)?.neighbor);
    }
    resolveProximityMatchesForSwap(graph) {
        if (!this.proximity.settings?.enabled)
            return [];
        try {
            const matches = Array.isArray(this.proximity.overlayMatches)
                ? this.proximity.overlayMatches
                : [];
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
        if (!this.layoutService)
            return null;
        const index = this.layoutService.buildIndex(graph);
        const order = this.layoutService.deriveOrder(index, {
            graph,
            anchorInstanceId: instanceId,
            proximityMatches: this.resolveProximityMatchesForSwap(graph),
        });
        const currentIndex = order.findIndex((entry) => entry?.instanceId === instanceId);
        const current = order[currentIndex];
        if (currentIndex < 0 || !this.canSwapEntry(current))
            return null;
        let neighbor = null;
        for (let nextIndex = currentIndex + direction; nextIndex >= 0 && nextIndex < order.length; nextIndex += direction) {
            const candidate = order[nextIndex];
            if (this.canSwapEntry(candidate)) {
                neighbor = candidate || null;
                break;
            }
        }
        if (!current?.instanceId || !neighbor?.instanceId)
            return null;
        return { order: order, current, neighbor };
    }
}
