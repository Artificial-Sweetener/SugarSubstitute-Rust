/** Keep ambiguous or non-model dropdowns under their original widget owner. */
export function recognizeModelField(snapshot, classType, name, options, origins = null) {
    if (origins !== null) {
        if (origins.length === 0)
            return null;
        const kinds = origins.map((origin) => recognizeModelField(snapshot, origin.classType, origin.inputName, options));
        return kinds.every((kind) => kind !== null && kind === kinds[0]) ? (kinds[0] ?? null) : null;
    }
    const declared = snapshot.fieldContracts.find((rule) => rule.classType === classType && rule.inputName === name) ?? snapshot.fieldContracts.find((rule) => rule.classType === '' && rule.inputName === name);
    if (declared)
        return declared.kind;
    const kinds = new Set(snapshot.items.filter((item) => options.includes(item.value)).map((item) => item.kind));
    return kinds.size === 1 ? ([...kinds][0] ?? null) : null;
}
