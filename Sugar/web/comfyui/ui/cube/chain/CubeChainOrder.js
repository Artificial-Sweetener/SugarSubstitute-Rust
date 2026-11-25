import { linkedOriginNodeIds } from '../../surface/CubeFaceCardPolicy.js';
/**
 * Order Cubes as they run. A Cube comes after every Cube it receives from, through a real link
 * (directly or through plain nodes) or a proximity link. Cubes with no dependency between them
 * read left to right, then top to bottom, then by instance id.
 */
export function cubeChainOrder(cubes, edges) {
    const byNode = new Map(cubes.map((cube) => [cube.nodeId, cube]));
    const sources = new Map();
    for (const edge of edges) {
        if (edge.source === edge.target)
            continue;
        const list = sources.get(edge.target);
        if (list)
            list.push(edge.source);
        else
            sources.set(edge.target, [edge.source]);
    }
    const upstream = new Map(cubes.map((cube) => [cube.instanceId, upstreamCubes(cube, byNode, sources)]));
    const placed = new Set();
    const remaining = [...cubes].sort(compareChainPosition);
    const order = [];
    while (remaining.length) {
        // A cycle leaves nothing ready; the leftmost remaining Cube then breaks it.
        const index = Math.max(0, remaining.findIndex((cube) => [...(upstream.get(cube.instanceId) ?? [])].every((id) => placed.has(id))));
        const [next] = remaining.splice(index, 1);
        if (!next)
            break;
        placed.add(next.instanceId);
        order.push(next.instanceId);
    }
    return order;
}
/** Read the live root graph's Cubes and dependencies, including the proximity links shown. */
export function readLiveChain(graph, cubeNodes, instanceIdOf, matches) {
    const cubes = cubeNodes.map((node) => ({
        instanceId: instanceIdOf(node),
        nodeId: String(node.id),
        position: [finite(node.pos?.[0]), finite(node.pos?.[1])],
    }));
    const edges = [];
    for (const node of graph._nodes ?? graph.nodes ?? []) {
        for (const origin of linkedOriginNodeIds(node, graph))
            edges.push({ source: String(origin), target: String(node.id) });
    }
    for (const match of matches) {
        const source = match.outputNode?.id ?? match.outputId;
        const target = match.inputNode?.id ?? match.inputId;
        if (source !== undefined && target !== undefined)
            edges.push({ source: String(source), target: String(target) });
    }
    return { cubes, edges };
}
/** Compare two Cubes by where they sit: left to right, then top to bottom, then id. */
export function compareChainPosition(left, right) {
    return (left.position[0] - right.position[0] ||
        left.position[1] - right.position[1] ||
        (left.instanceId < right.instanceId ? -1 : left.instanceId > right.instanceId ? 1 : 0));
}
/** Find the nearest Cubes a Cube receives from, walking back through plain nodes. */
function upstreamCubes(cube, byNode, sources) {
    const found = new Set();
    const seen = new Set([cube.nodeId]);
    const pending = [...(sources.get(cube.nodeId) ?? [])];
    while (pending.length) {
        const id = pending.pop();
        if (id === undefined || seen.has(id))
            continue;
        seen.add(id);
        const other = byNode.get(id);
        if (other)
            found.add(other.instanceId);
        else
            pending.push(...(sources.get(id) ?? []));
    }
    return found;
}
/** Read one finite coordinate. */
function finite(value) {
    const number = Number(value);
    return Number.isFinite(number) ? number : 0;
}
