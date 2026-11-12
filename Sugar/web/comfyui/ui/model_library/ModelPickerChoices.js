/** Retain every host option, including sentinels and files without provider metadata. */
export function modelPickerChoices(items, kind, options, optionLabel) {
    const available = new Map(items.filter((item) => item.kind === kind).map((item) => [item.value, item]));
    return options.map((value) => available.get(value) ?? literalModelChoice(kind, value, optionLabel?.(value)));
}
/** Present labeled host references at the root without interpreting opaque identifiers as paths. */
function literalModelChoice(kind, value, label) {
    const path = value.replaceAll('\\', '/');
    const filename = path.split('/').at(-1) ?? path;
    const labeled = Boolean(label && label !== value);
    return {
        id: `literal:${kind}:${value}`,
        kind,
        value,
        rootId: '',
        relativePath: labeled ? '' : path,
        folder: !labeled && path.includes('/') ? path.slice(0, path.lastIndexOf('/')) : '',
        title: label && label !== value
            ? label
            : filename.replace(/\.(?:safetensors|ckpt|pt|pth|bin|gguf)$/i, ''),
        version: '',
        baseModel: '',
        provider: '',
        providerUrl: '',
        providerModelId: '',
        providerVersionId: '',
        providerLinks: [],
        description: '',
        trainedWords: [],
        thumbnails: [],
    };
}
/** Filter within the active folder using all of Substitute's useful searchable labels. */
export function filterModelChoices(items, query, folder) {
    const terms = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
    return items.filter((item) => (folder === '' || item.folder === folder || item.folder.startsWith(`${folder}/`)) &&
        terms.every((term) => [item.title, item.version, item.value, item.baseModel, ...item.trainedWords]
            .join(' ')
            .toLocaleLowerCase()
            .includes(term)));
}
/** Return immediate child folders without maintaining a second mutable folder index. */
export function modelChoiceFolders(items, folder) {
    const prefix = folder ? `${folder}/` : '';
    const children = new Set();
    for (const item of items) {
        if (!item.folder.startsWith(prefix))
            continue;
        const child = item.folder.slice(prefix.length).split('/')[0];
        if (child)
            children.add(`${prefix}${child}`);
    }
    return [...children].sort((left, right) => left.localeCompare(right));
}
