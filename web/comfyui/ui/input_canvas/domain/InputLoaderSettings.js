/** Exclude the file field and reject dynamic/nonfinite values before crossing the native interpretation boundary. */
export function inputLoaderSettings(loader, values) {
    const inputs = {};
    for (const [name, value] of Object.entries(values))
        if (name !== loader.field &&
            (typeof value === 'string' ||
                typeof value === 'boolean' ||
                (typeof value === 'number' && Number.isFinite(value))))
            inputs[name] = value;
    return Object.freeze({
        kind: loader.kind,
        classType: loader.classType,
        field: loader.field,
        inputs: Object.freeze(inputs),
    });
}
