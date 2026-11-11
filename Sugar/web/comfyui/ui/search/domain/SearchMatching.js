/** Find non-overlapping literal matches while retaining UTF-16 source offsets for native selection. */
export function findSearchRanges(source, needle) {
    if (!needle)
        return [];
    const folded = source.toLowerCase();
    const positions = [];
    if (folded.length !== source.length) {
        let offset = 0;
        for (const character of source) {
            positions.push(...Array.from({ length: character.toLowerCase().length }, () => ({
                start: offset,
                end: offset + character.length,
            })));
            offset += character.length;
        }
    }
    const ranges = [];
    for (let index = folded.indexOf(needle); index >= 0; index = folded.indexOf(needle, index + needle.length)) {
        const start = positions[index]?.start ?? index;
        const end = positions[index + needle.length - 1]?.end ?? index + needle.length;
        if (ranges.at(-1)?.start !== start)
            ranges.push({ start, length: end - start });
    }
    return ranges;
}
/** Apply Field's all-token matching, Node's substring matching, and scoped quoted value matching. */
export function matchSearch(nodes, query) {
    const matchingNodes = new Set();
    const matchingFields = new Set();
    const matches = [];
    for (const node of nodes) {
        if (query.mode === 'field') {
            for (const field of node.fields)
                if (query.tokens.every((token) => `${field.key} ${field.label}`.toLowerCase().includes(token))) {
                    matchingFields.add(field.id);
                    matchingNodes.add(node.id);
                }
        }
        else if (query.mode === 'node') {
            const labels = [
                node.name,
                node.type,
                node.title,
                ...node.fields.flatMap((field) => [field.key, field.label]),
            ];
            if (!query.node || labels.some((label) => label.toLowerCase().includes(query.node)))
                matchingNodes.add(node.id);
        }
        if (!query.text || (query.mode === 'node' && query.node && !matchingNodes.has(node.id)))
            continue;
        for (const field of node.fields)
            if (field.text !== null)
                for (const range of findSearchRanges(field.text, query.text))
                    matches.push({ node: node.id, field: field.id, ...range });
    }
    return { query, nodes: matchingNodes, fields: matchingFields, matches };
}
