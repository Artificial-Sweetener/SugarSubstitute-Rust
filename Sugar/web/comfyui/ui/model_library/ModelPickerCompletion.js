/** Complete the highlighted filename even before typing, then honor path and friendly-name searches. */
export function modelChoiceCompletion(item, query) {
    if (!item)
        return null;
    const path = (item.relativePath.trim() || item.value.trim()).replaceAll('\\', '/');
    const stripped = path.replace(/\.(?:safetensors|ckpt|pt)$/i, '');
    const filename = stripped.split('/').at(-1) ?? stripped;
    const normalized = query.toLocaleLowerCase();
    const hasSeparator = /[\\/]/.test(query);
    if (hasSeparator && stripped.toLocaleLowerCase().startsWith(normalized.replaceAll('\\', '/'))) {
        const candidate = query.includes('\\') && !query.includes('/') ? stripped.replaceAll('/', '\\') : stripped;
        return completion(candidate.slice(query.length), query);
    }
    if (filename.toLocaleLowerCase().startsWith(normalized))
        return completion(filename.slice(query.length), query);
    if (!hasSeparator && query.trim().length > 0) {
        const start = filename.toLocaleLowerCase().indexOf(normalized);
        if (start > 0)
            return completion(filename.slice(start + query.length), query);
    }
    const friendly = [item.title.trim(), item.version.trim()].filter(Boolean).join(' - ');
    if (friendly.toLocaleLowerCase().startsWith(normalized))
        return completion(friendly.slice(query.length), query);
    return filename ? { suffix: ` → ${filename}`, text: filename } : null;
}
/** Accept ordinary completions by extending typed search, leaving exact matches unadorned. */
function completion(suffix, query) {
    return suffix ? { suffix, text: query + suffix } : null;
}
