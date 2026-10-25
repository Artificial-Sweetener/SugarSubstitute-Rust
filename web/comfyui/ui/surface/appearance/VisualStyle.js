/** Accept only styles with an installed presentation implementation. */
export function isVisualStyle(value) {
    return (value === 'comfy' ||
        value === 'fluent' ||
        value === 'spectrum' ||
        value === 'photopro' ||
        value === 'gimp');
}
/** Preserve the public Fluent preference until a visual style has been explicitly saved. */
export function resolveVisualStyle(value, legacyFluent) {
    return isVisualStyle(value) ? value : legacyFluent === true ? 'fluent' : 'comfy';
}
