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
/** Adapt Comfy graph links and exact boundary-slot geometry for proximity policy. */
/** Own link resolution and surface-slot positions at the Comfy adapter boundary. */
export class ComfyGraphGeometry {
    #logger;
    #slotPositionBuffer = new Float32Array(2);
    /** Bind actionable logging for dynamic host failures. */
    constructor(logger) {
        this.#logger = logger;
    }
    /** Resolve one live graph link across Comfy's supported link containers. */
    resolveLink(graph, linkId) {
        if (!graph || linkId == null)
            return null;
        if (typeof graph.getLink === 'function')
            return graph.getLink(linkId) ?? null;
        const links = graph.links;
        if (links instanceof Map)
            return links.get(linkId) ?? null;
        if (Array.isArray(links)) {
            return links.find((link) => String(link.id) === String(linkId)) ?? null;
        }
        return links?.[String(linkId)] ?? null;
    }
    /** Return the first live link on one input or output slot. */
    firstLiveLink(graph, slot) {
        for (const linkId of readLinkIds(slot)) {
            const link = this.resolveLink(graph, linkId);
            if (link)
                return link;
        }
        return null;
    }
    /** Read Comfy's exact connection position for any boundary slot index. */
    slotPosition(node, isOutput, slot) {
        try {
            if (typeof node.getSlotPosition === 'function') {
                const value = node.getSlotPosition(slot, !isOutput);
                if (isFinitePoint(value))
                    return [Number(value[0]), Number(value[1])];
            }
            if (typeof node.getConnectionPos === 'function') {
                const isInput = !isOutput;
                const value = node.getConnectionPos(isInput, slot, this.#slotPositionBuffer);
                if (isFinitePoint(this.#slotPositionBuffer)) {
                    return [this.#slotPositionBuffer[0] ?? 0, this.#slotPositionBuffer[1] ?? 0];
                }
                if (isFinitePoint(value))
                    return [Number(value[0]), Number(value[1])];
            }
        }
        catch (error) {
            this.#logger.warn('SugarCubes failed to read a Cube boundary position.', {
                nodeId: node.id,
                slot,
                direction: isOutput ? 'output' : 'input',
                error,
            });
        }
        const x = Number(node.pos?.[0]) || 0;
        const y = Number(node.pos?.[1]) || 0;
        const width = Number(node.size?.[0]) || 0;
        const height = Number(node.size?.[1]) || 0;
        return [isOutput ? x + width : x, y + height / 2];
    }
}
/** Normalize every usable link id stored by LiteGraph slots. */
function readLinkIds(slot) {
    if (!slot)
        return [];
    const values = [...(Array.isArray(slot.links) ? slot.links : []), slot.link];
    return values.filter((value) => {
        if (typeof value === 'string')
            return value.trim().length > 0;
        return typeof value === 'number' && Number.isFinite(value) && value >= 0;
    });
}
/** Narrow arrays and typed arrays to a finite two-dimensional point. */
function isFinitePoint(value) {
    if (!Array.isArray(value) &&
        !(value instanceof Float32Array) &&
        !(value instanceof Float64Array)) {
        return false;
    }
    return Number.isFinite(Number(value[0])) && Number.isFinite(Number(value[1]));
}
