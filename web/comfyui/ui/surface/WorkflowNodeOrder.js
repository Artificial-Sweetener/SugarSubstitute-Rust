import { orderNodesUpstreamFirst } from './CubeFaceCardPolicy.js';
/** Resolve dependency order with spatial reading order for otherwise independent nodes. */
export function orderWorkflowNodes(graph, connections = []) {
    const nodes = [...(graph._nodes ?? graph.nodes ?? [])].sort((a, b) => (a.pos?.[0] ?? 0) - (b.pos?.[0] ?? 0) || (a.pos?.[1] ?? 0) - (b.pos?.[1] ?? 0));
    const byId = new Map(nodes.map((node) => [String(node.id), node]));
    return orderNodesUpstreamFirst(nodes, graph, connections).flatMap((id) => {
        const node = byId.get(id);
        return node ? [node] : [];
    });
}
