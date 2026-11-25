import { orderNodesUpstreamFirst } from './CubeFaceCardPolicy.js';
import { isCubeNode, requireCubeIdentity } from '../cube/node/ComfyCubeNodeFactory.js';
/**
 * Read nodes upstream first and fill the Cube places with the stack order, so masonry, the stack
 * and every inventory list Cubes the way the Cube chain reads them.
 */
export function orderWorkflowNodes(graph, connections = [], cubeOrder = null) {
    const nodes = [...(graph._nodes ?? graph.nodes ?? [])].sort((a, b) => (a.pos?.[0] ?? 0) - (b.pos?.[0] ?? 0) || (a.pos?.[1] ?? 0) - (b.pos?.[1] ?? 0));
    const byId = new Map(nodes.map((node) => [String(node.id), node]));
    const ordered = orderNodesUpstreamFirst(nodes, graph, connections).flatMap((id) => {
        const node = byId.get(id);
        return node ? [node] : [];
    });
    if (!cubeOrder)
        return ordered;
    const byInstance = new Map(nodes
        .filter(isCubeNode)
        .map((cube) => [String(requireCubeIdentity(cube).instance_id ?? cube.id), cube]));
    const stack = cubeOrder.flatMap((id) => {
        const cube = byInstance.get(id);
        return cube ? [cube] : [];
    });
    if (stack.length !== byInstance.size)
        return ordered;
    let cubeIndex = 0;
    return ordered.map((node) => (isCubeNode(node) ? (stack[cubeIndex++] ?? node) : node));
}
