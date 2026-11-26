import { autocompleteListId, autocompleteListName, listNameRefusal, } from '../domain/AutocompleteList.js';
const GROUPS = { custom: 'Custom tag lists', censored: 'Censored tag lists' };
/**
 * Custom and censored tag lists for the editor. Every change goes through the library, which
 * republishes the tags, so suggestions change as soon as a list is saved or turned on or off.
 */
export function autocompleteListTextAssets(library) {
    const refusal = (kind, typed, except) => {
        const name = autocompleteListName(typed);
        const unsafe = listNameRefusal(name);
        if (unsafe)
            return unsafe;
        const id = autocompleteListId(kind, name);
        return library.lists.some((list) => list.id === id && list.id !== except)
            ? `Autocomplete list already exists: ${name}`
            : null;
    };
    const kindOf = (asset) => asset.id.startsWith('censored/') ? 'censored' : 'custom';
    const creation = (kind) => ({
        label: kind === 'custom' ? 'New custom list' : 'New censored list',
        refusal: (name) => refusal(kind, name, null),
        create: (name) => library.create(kind, autocompleteListName(name)),
    });
    return {
        title: 'Autocomplete Lists',
        listTitle: 'Tag lists',
        emptyText: 'No custom or censored tag lists yet.',
        creations: [creation('custom'), creation('censored')],
        async assets() {
            await library.refresh();
            return library.lists.map((list) => {
                const count = library.tagCount(list.id);
                return {
                    id: list.id,
                    group: GROUPS[list.kind],
                    label: list.name,
                    subtitle: `${String(count)} ${count === 1 ? 'tag' : 'tags'} · ${list.enabled ? 'Enabled' : 'Disabled'}`,
                    enabled: list.enabled,
                    document: 'tag-list',
                };
            });
        },
        read: (id) => library.read(id),
        write: (id, text) => library.write(id, text),
        renameText: (asset) => asset.label,
        renameRefusal: (asset, name) => refusal(kindOf(asset), name, asset.id),
        rename: (asset, name) => library.rename(asset.id, autocompleteListName(name)),
        delete: (asset) => library.delete(asset.id),
        setEnabled: (asset, enabled) => library.setEnabled(asset.id, enabled),
        applied: () => library.refresh(),
    };
}
