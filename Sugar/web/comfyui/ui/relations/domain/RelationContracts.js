/** Create isolated workflow state for unconfigured relation metadata. */
export function emptyRelationState() {
    return {
        schema: 1,
        overrides: {},
        fieldLinks: {},
        nodeLinks: {},
        locals: {},
        localSeedModes: {},
        localNodeModes: {},
    };
}
