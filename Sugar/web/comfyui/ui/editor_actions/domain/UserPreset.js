/** Preserve the shared library's global association identity. */
export const GLOBAL_ASSOCIATION = {
    scope: 'global',
    provider: null,
    key: 'global',
    label: 'Global',
};
/** Compare persisted identities rather than translated presentation labels. */
export function sameAssociation(left, right) {
    return left.scope === right.scope && left.provider === right.provider && left.key === right.key;
}
