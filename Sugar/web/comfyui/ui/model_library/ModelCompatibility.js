/** Use server decisions for addons and exact declared architecture/family requirements for models. */
export function modelCompatibility(item, context) {
    const traits = item.traits;
    if (traits?.conflict)
        return 'unknown';
    const architecture = traits?.architecture || item.architecture?.base || '';
    if (context.addon && context.family) {
        const support = traits?.compatibleTargets.find((target) => target.family === context.family)?.support;
        return support ?? (traits?.family ? 'incompatible' : 'unknown');
    }
    if (context.architecture && !architecture)
        return 'unknown';
    if (context.architecture && architecture !== context.architecture)
        return 'incompatible';
    if (context.family && !traits?.family)
        return 'unknown';
    if (context.family && traits?.family !== context.family)
        return 'incompatible';
    // Architecture alone cannot establish release-specific addon compatibility.
    return context.addon ? 'unknown' : 'full';
}
/** Expose filters for existing pickers and a future browser without creating a browser surface. */
export function queryModelCatalog(items, query) {
    const terms = query.text?.trim().toLowerCase().split(/\s+/).filter(Boolean) ?? [];
    return items.filter((item) => (!query.kind || item.kind === query.kind) &&
        (!query.architecture ||
            (item.traits?.architecture || item.architecture?.base) === query.architecture) &&
        (!query.family || item.traits?.family === query.family) &&
        (!query.nsfw || (item.traits?.nsfw ?? 'unknown') === query.nsfw) &&
        (!query.tags ||
            query.tags.every((tag) => item.tags?.some((value) => value.toLowerCase() === tag.toLowerCase()))) &&
        terms.every((term) => [
            item.value,
            item.title,
            item.baseModel,
            item.traits?.familyName ?? '',
            ...item.trainedWords,
            ...(item.tags ?? []),
        ]
            .join(' ')
            .toLowerCase()
            .includes(term)) &&
        (!query.compatibleWith ||
            ['full', 'partial'].includes(modelCompatibility(item, query.compatibleWith))));
}
/** Preserve a compatible selection; choose a full match before a partial match for missing models. */
export function resolveInstalledModel(items, current, context) {
    const compatible = queryModelCatalog(items, { compatibleWith: context });
    return (compatible.find((item) => item.value === current) ??
        compatible.find((item) => modelCompatibility(item, context) === 'full') ??
        compatible[0] ??
        null);
}
