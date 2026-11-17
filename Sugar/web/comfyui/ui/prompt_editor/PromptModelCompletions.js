/**
 * Adapt the model library's gallery to the editor's candidate popup.
 *
 * The editor ranks LoRAs by their installed identity; `model` resolves each identity to the
 * catalog record the gallery draws. A candidate the catalog no longer holds is left out, and the
 * positions the editor and the gallery speak in are translated both ways.
 */
export function promptModelCompletions(gallery, model) {
    return {
        mount(events) {
            // Gallery position -> candidate position, for the candidates currently shown.
            let candidates = [];
            const surface = gallery.mountGallery({
                activate: (index) => {
                    const candidate = candidates[index];
                    if (candidate !== undefined)
                        events.activate(candidate);
                },
            });
            return {
                element: surface.element,
                update(models, selected) {
                    const items = [];
                    const shown = [];
                    for (const [index, candidate] of models.entries()) {
                        const item = model(candidate.model.backendValue);
                        if (!item)
                            continue;
                        items.push(item);
                        shown.push(index);
                    }
                    candidates = shown;
                    surface.update(items, Math.max(0, shown.indexOf(selected)));
                },
                neighbor(selected, key) {
                    const active = candidates.indexOf(selected);
                    if (active < 0)
                        return selected;
                    return candidates[surface.neighbor(active, key)] ?? selected;
                },
                dispose: () => surface.dispose(),
            };
        },
    };
}
