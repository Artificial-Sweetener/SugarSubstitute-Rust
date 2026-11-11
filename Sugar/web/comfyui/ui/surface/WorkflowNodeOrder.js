import { orderNodesUpstreamFirst } from './CubeFaceCardPolicy.js';
import { isCubeNode, requireCubeIdentity } from '../cube/node/ComfyCubeNodeFactory.js';
import { readNativeCubePosition } from '../cube/graph/NativeCubePositions.js';
import { cubePlacementOrder } from '../workflow/domain/CubeGraphOrder.js';
/** Keep loose-node dependencies while reading Cube slots in their canonical native position order. */
export function orderWorkflowNodes(graph, connections = []) {
    const nodes = [...(graph._nodes ?? graph.nodes ?? [])].sort((a, b) => (a.pos?.[0] ?? 0) - (b.pos?.[0] ?? 0) || (a.pos?.[1] ?? 0) - (b.pos?.[1] ?? 0));
    const byId = new Map(nodes.map((node) => [String(node.id), node]));
    const ordered = orderNodesUpstreamFirst(nodes, graph, connections).flatMap((id) => {
        const node = byId.get(id);
        return node ? [node] : [];
    });
    const cubes = nodes.filter(isCubeNode);
    const byInstance = new Map(cubes.map((cube) => [String(requireCubeIdentity(cube).instance_id ?? cube.id), cube]));
    const cubeOrder = cubePlacementOrder(cubes.map((cube) => ({
        instanceId: String(requireCubeIdentity(cube).instance_id ?? cube.id),
        nodeId: String(cube.id),
        position: readNativeCubePosition(cube.pos),
    })));
    let cubeIndex = 0;
    return ordered.map((node) => {
        if (!isCubeNode(node))
            return node;
        const id = cubeOrder[cubeIndex++];
        return (id === undefined ? undefined : byInstance.get(id)) ?? node;
    });
}
