import { commonFieldDomain } from './FieldCompatibility.js';
import { linkedFieldMembers, resolveRelations } from './RelationResolver.js';
/** Update globals and linked masters once while leaving native independent seeds to Comfy. */
export function advanceSharedSeeds(fields, nodes, groups, state, step) {
    for (const group of groups) {
        if (!group.enabled || group.members[0]?.semantic !== 'seed')
            continue;
        const override = state.overrides[group.key];
        if (override)
            override.value = advanceSeed(group.domain, group.value, group.seedMode, step);
    }
    const resolved = resolveRelations(fields, nodes, state, groups);
    for (const item of resolved) {
        if (item.field.semantic !== 'seed' || item.override || item.source)
            continue;
        const members = linkedFieldMembers(item.field.id, fields, nodes, state);
        if (members.length < 2)
            continue;
        const mode = state.localSeedModes[item.field.id] ?? item.field.seedMode ?? 'fixed';
        state.locals[item.field.id] = advanceSeed(commonFieldDomain(members), item.value, mode, step);
    }
}
/**
 * Give one shared seed its next value by the host's rule, so a shared seed and an unshared one
 * are rolled and stepped identically.
 *
 * A seed beyond a page number's exact range was not rolled here: it came from an image or was
 * typed. One step from it cannot be held, so stepping leaves it as it is.
 */
function advanceSeed(domain, current, mode, step) {
    if (domain?.kind !== 'number' || typeof current !== 'number' || mode === 'fixed')
        return current;
    if (mode !== 'randomize' && !Number.isSafeInteger(current))
        return current;
    return step(domain, current, mode);
}
