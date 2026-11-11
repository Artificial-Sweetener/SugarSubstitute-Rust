/** Consume a completed context command, keeping incomplete commands out of matching. */
export function parseSearchInput(mode, text) {
    const command = /^\s*@(text|field|node)\s+/i.exec(text);
    if (command) {
        const context = command[1]?.toLowerCase();
        if (context === 'text' || context === 'field' || context === 'node')
            return { mode: context, text: text.slice(command[0].length).trimStart(), pending: false };
    }
    return { mode, text, pending: text.trimStart().startsWith('@') };
}
/** Split a Node query at its first quoted text portion, accepting an unfinished closing quote. */
export function buildSearchQuery(mode, raw) {
    const normalized = raw.trim().toLowerCase();
    const quote = raw.indexOf('"');
    const node = mode === 'node' ? (quote < 0 ? raw : raw.slice(0, quote)).trim().toLowerCase() : '';
    const after = quote < 0 ? '' : raw.slice(quote + 1);
    const close = after.indexOf('"');
    const text = mode === 'text'
        ? normalized
        : mode === 'node'
            ? (close < 0 ? after : after.slice(0, close)).trim().toLowerCase()
            : '';
    return {
        mode,
        raw,
        normalized,
        node,
        text,
        tokens: (mode === 'node' ? node : normalized).split(/\s+/).filter(Boolean),
    };
}
