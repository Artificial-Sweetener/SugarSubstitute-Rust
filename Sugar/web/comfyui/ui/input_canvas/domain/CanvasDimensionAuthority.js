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
/** Find the authored width and height that size a mask with no paired image. */
const SPATIAL_TYPES = new Set(['IMAGE', 'LATENT', 'MASK']);
const SOURCE_TYPES = new Set(['IMAGE', 'LATENT']);
/** Pick exactly one `width`/`height` or `<stem>_width`/`<stem>_height` pair from field names. */
export function inferDimensionFieldPair(fields) {
    const widths = new Map(), heights = new Map();
    for (const field of fields) {
        const match = /^(?:(.+)_)?(width|height)$/.exec(field);
        if (!match)
            continue;
        const stem = match[1] ?? '', side = match[2] === 'width' ? widths : heights;
        if (!side.has(stem))
            side.set(stem, field);
    }
    const pairs = [...widths].flatMap(([stem, width]) => {
        const height = heights.get(stem);
        return height ? [{ width, height }] : [];
    });
    return pairs.length === 1 ? (pairs[0] ?? null) : null;
}
/**
 * Resolve the spatial root that sizes a mask.
 *
 * The mask's downstream reach is searched from its used outputs. A spatial root is a node with no
 * spatial input connection; each of its IMAGE or LATENT outputs whose downstream reach meets the
 * mask's is a candidate when the node has one authored positive integer width/height pair. Latent
 * candidates win over image candidates; candidates that disagree on size are ambiguous rather
 * than guessed. Agreeing candidates all become authorities so a resize writes every one of them.
 */
export function resolveDimensionAuthority(topology, mask, value) {
    const outgoing = new Map();
    const spatialInputs = new Set();
    for (const edge of topology.edges) {
        const edges = outgoing.get(edge.source) ?? [];
        edges.push(edge);
        outgoing.set(edge.source, edges);
        if (SPATIAL_TYPES.has(edge.type ?? '') || SPATIAL_TYPES.has(edge.inputType ?? ''))
            spatialInputs.add(edge.target);
    }
    const reach = (edges) => {
        const distances = new Map();
        let frontier = [...new Set(edges.map((edge) => edge.target))];
        for (let distance = 1; frontier.length; distance++) {
            const next = [];
            for (const node of frontier) {
                if (distances.has(node))
                    continue;
                distances.set(node, distance);
                for (const edge of outgoing.get(node) ?? [])
                    next.push(edge.target);
            }
            frontier = next;
        }
        return distances;
    };
    const maskReach = reach((outgoing.get(mask) ?? []).filter((edge) => (edge.type ?? '').toUpperCase() === 'MASK'));
    if (!maskReach.size)
        return { kind: 'missing', reason: 'mask_output_is_unused' };
    const candidates = [];
    for (const node of topology.nodes) {
        if (node.key === mask || spatialInputs.has(node.key) || node.dimensions.length !== 1)
            continue;
        const pair = node.dimensions[0];
        if (!pair)
            continue;
        const width = value(node.key, pair.width), height = value(node.key, pair.height);
        if (!positiveInteger(width) || !positiveInteger(height))
            continue;
        for (const [output, type] of node.outputs.entries()) {
            if (!type || !SOURCE_TYPES.has(type))
                continue;
            const sourceReach = reach((outgoing.get(node.key) ?? []).filter((edge) => edge.output === output));
            let distance = Number.POSITIVE_INFINITY;
            for (const [target, steps] of sourceReach) {
                const maskSteps = maskReach.get(target);
                if (maskSteps !== undefined)
                    distance = Math.min(distance, steps + maskSteps);
            }
            if (Number.isFinite(distance))
                candidates.push({ node: node.key, output, ...pair, type, distance, size: [width, height] });
        }
    }
    if (!candidates.length)
        return { kind: 'missing', reason: 'no_relevant_spatial_root_with_dimensions' };
    const preferred = candidates.some((candidate) => candidate.type === 'LATENT')
        ? candidates.filter((candidate) => candidate.type === 'LATENT')
        : candidates;
    preferred.sort((left, right) => left.distance - right.distance ||
        (left.node < right.node ? -1 : left.node > right.node ? 1 : 0) ||
        left.output - right.output);
    if (new Set(preferred.map((candidate) => candidate.size.join('x'))).size > 1)
        return {
            kind: 'ambiguous',
            reason: 'relevant_spatial_roots_disagree',
            candidates: [...new Set(preferred.map((candidate) => candidate.node))],
        };
    const first = preferred[0];
    if (!first)
        return { kind: 'missing', reason: 'no_relevant_spatial_root_with_dimensions' };
    const nodes = preferred.map(({ node, output, width, height }) => ({
        node,
        output,
        width,
        height,
    }));
    return {
        kind: 'resolved',
        key: JSON.stringify(nodes.map((item) => [item.node, item.output, item.width, item.height])),
        width: first.size[0],
        height: first.size[1],
        nodes,
    };
}
/** Admit authored integers only; linked or boolean values never size a canvas. */
function positiveInteger(value) {
    return typeof value === 'number' && Number.isSafeInteger(value) && value > 0;
}
