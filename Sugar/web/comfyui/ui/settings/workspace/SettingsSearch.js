/** Split text into searchable words: runs of word characters, braces and `#`. */
export function searchWords(text) {
    return text
        .toLocaleLowerCase()
        .split(/[^\p{L}\p{N}_{}#]+/u)
        .filter(Boolean);
}
/** Every row of the searchable pages that the query matches, in page, section and row order. */
export function searchSettings(pages, query) {
    const wanted = searchWords(query);
    if (!wanted.length)
        return [];
    const results = [];
    for (const page of pages) {
        if (page.searchable === false)
            continue;
        for (const section of page.sections)
            for (const row of section.rows) {
                if (row.body || row.visible?.read() === false)
                    continue;
                const words = searchWords([
                    page.title,
                    page.subtitle,
                    section.title,
                    section.subtitle ?? '',
                    row.title,
                    row.description ?? '',
                    ...(row.keywords ?? []),
                ].join(' '));
                if (wanted.every((word) => words.some((candidate) => candidate.startsWith(word))))
                    results.push({ page, section, row });
            }
    }
    return results;
}
