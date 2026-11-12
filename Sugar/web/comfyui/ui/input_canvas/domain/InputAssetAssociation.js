/** Associate masks only when every image-bearing consumer resolves to one compatible image field. */
export function resolveInputAssetAssociations(graphs) {
    const endpoints = new Map(graphs.flatMap((graph) => graph.endpoints.map((endpoint) => [endpoint.key, endpoint])));
    const consumers = new Map();
    for (const graph of graphs)
        for (const input of graph.inputs) {
            const inputs = consumers.get(input.consumer) ?? [];
            if (!inputs.some((previous) => previous.input === input.input))
                inputs.push(input);
            consumers.set(input.consumer, inputs);
        }
    const masks = new Map();
    for (const endpoint of endpoints.values())
        if (endpoint.kind === 'mask')
            masks.set(endpoint.key, {
                images: new Set(),
                consumers: new Set(),
                unresolved: false,
                ambiguous: false,
            });
    for (const [consumer, inputs] of consumers) {
        const images = inputs.filter((input) => input.kind === 'image');
        if (!images.length)
            continue;
        const candidates = [
            ...new Set(images
                .flatMap((input) => input.endpoints)
                .filter((key) => endpoints.get(key)?.kind === 'image')),
        ];
        for (const input of inputs.filter((input) => input.kind === 'mask'))
            for (const key of input.endpoints) {
                const mask = masks.get(key);
                if (!mask)
                    continue;
                mask.consumers.add(consumer);
                for (const candidate of candidates)
                    mask.images.add(candidate);
                mask.ambiguous ||= images.length > 1 || candidates.length > 1;
                mask.unresolved ||= candidates.length !== 1;
            }
    }
    return new Map([...masks].map(([mask, facts]) => {
        const candidates = [...facts.images].sort(), consumers = [...facts.consumers].sort();
        const image = candidates[0];
        const association = image && candidates.length === 1 && !facts.ambiguous && !facts.unresolved
            ? { kind: 'associated', mask, image, consumers }
            : {
                kind: facts.ambiguous ||
                    candidates.length > 1 ||
                    (facts.unresolved && candidates.length > 0)
                    ? 'ambiguous'
                    : 'missing',
                mask,
                candidates,
                consumers,
            };
        return [mask, Object.freeze(association)];
    }));
}
/** Admit editing only when no other mask can target the same image input. */
export function exclusiveInputImage(associations, mask, maskSources) {
    const pair = associations.get(mask);
    if (pair?.kind !== 'associated' || maskSources.has(pair.image))
        return null;
    for (const other of associations.values()) {
        if (other.mask === mask)
            continue;
        if (other.kind === 'associated'
            ? other.image === pair.image
            : other.candidates.includes(pair.image))
            return null;
    }
    return pair.image;
}
