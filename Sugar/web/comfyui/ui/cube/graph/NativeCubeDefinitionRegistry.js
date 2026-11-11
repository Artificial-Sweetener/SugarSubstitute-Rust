/** Remove an allocated native definition and its registered node type together. */
export function discardNativeCubeDefinition(graph, liteGraph, id) {
    graph.subgraphs.delete(id);
    if (typeof liteGraph.unregisterNodeType === 'function')
        liteGraph.unregisterNodeType.call(liteGraph, id);
}
