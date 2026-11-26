/**
 * Select models from the catalog and keep the selection current. A refreshed model, or an image
 * that arrives, reaches the surface without the surface being reopened.
 */
export function liveCatalogModels(catalog, held, select, reportError) {
    let shown = select(held);
    return {
        current: () => shown,
        watch: (changed) => catalog.watch((next) => {
            shown = select(next);
            changed();
        }, reportError),
    };
}
