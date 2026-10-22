/** Present exposed editable controls without classifying their boundary link as consumption. */
export function presentCubeCardNode(node, sources) {
    if (!sources.length)
        return node;
    const exposed = new Set(sources.map((source) => source.targetName));
    return {
        ...node,
        inputs: (node.inputs ?? []).filter((input) => !exposed.has(input.widget?.name ?? '')),
    };
}
/** Forward exposed widgets into their internal node cards while retaining host identity. */
export function resolveCubeCardWidgetSources(cube, node) {
    const sources = [];
    for (const input of node.inputs ?? []) {
        if (input.link == null || !input.widget?.name)
            continue;
        const link = cube.subgraph.getLink?.(input.link);
        if (!link || String(link.origin_id) !== String(cube.subgraph.inputNode.id))
            continue;
        if (typeof link.origin_slot !== 'number')
            continue;
        const boundary = cube.inputs?.[link.origin_slot];
        const name = boundary?.widget?.name;
        if (!name || boundary.link != null || (boundary.links?.length ?? 0) > 0)
            continue;
        sources.push({ node: cube, name, targetName: input.widget.name });
    }
    return sources;
}
