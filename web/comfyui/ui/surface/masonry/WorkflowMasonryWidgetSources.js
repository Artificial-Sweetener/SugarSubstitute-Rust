/** Forward exposed widgets into their internal node cards while retaining host identity. */
export function resolveMasonryWidgetSources(cube, node) {
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
