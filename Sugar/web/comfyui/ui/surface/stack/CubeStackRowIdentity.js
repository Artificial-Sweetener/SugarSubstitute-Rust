/**
 * Give the rail and the cart the same reading of a Cube: its name without the model route, the
 * model as the chip on its icon, its version and pack beneath the name, and the full title,
 * definition and source in the tooltip.
 */
export function cubeStackRowIdentity(identity, cubeId) {
    return {
        label: identity.instanceModelTitle.nameText,
        secondary: [identity.versionText, formatStackPackText(cubeId)].filter(Boolean).join(' · '),
        tooltip: [identity.instanceTitle, identity.definitionLine, identity.sourceLine]
            .filter(Boolean)
            .join('\n'),
        icon: identity.icon,
        targetModel: identity.targetModel,
        severity: identity.awaitingFirstSave ? 'warning' : 'none',
    };
}
/** Show the compact repository or local namespace retained by canonical and legacy Cube ids. */
function formatStackPackText(cubeId) {
    if (typeof cubeId !== 'string')
        return '';
    const parts = cubeId
        .replace(/\\/g, '/')
        .split('/')
        .map((part) => part.trim())
        .filter(Boolean);
    if (parts.length < 2)
        return '';
    const pack = parts.length >= 3 ? parts[1] : parts[0];
    return (pack ?? '')
        .replace(/[\s_]+/g, '-')
        .replace(/-+/g, '-')
        .replace(/^-|-$/g, '')
        .toLowerCase();
}
