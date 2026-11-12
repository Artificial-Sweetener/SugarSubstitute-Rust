import { commonFieldDomain } from './FieldCompatibility.js';
import { linkedFieldMembers, resolveRelations } from './RelationResolver.js';
/** Update globals and linked masters once while leaving native independent seeds to Comfy. */
export function advanceSharedSeeds(fields, nodes, groups, state, random) {
    for (const group of groups) {
        if (!group.enabled || group.members[0]?.semantic !== 'seed')
            continue;
        const override = state.overrides[group.key];
        if (override)
            override.value = advanceSeed(group.domain, group.value, group.seedMode, random);
    }
    const resolved = resolveRelations(fields, nodes, state, groups);
    for (const item of resolved) {
        if (item.field.semantic !== 'seed' || item.override || item.source)
            continue;
        const members = linkedFieldMembers(item.field.id, fields, nodes, state);
        if (members.length < 2)
            continue;
        const mode = state.localSeedModes[item.field.id] ?? item.field.seedMode ?? 'fixed';
        state.locals[item.field.id] = advanceSeed(commonFieldDomain(members), item.value, mode, random);
    }
}
/** Keep every generated value on the common step grid and wrap within its safe integer range. */
function advanceSeed(domain, current, mode, random) {
    if (domain?.kind !== 'number' || typeof current !== 'number' || mode === 'fixed')
        return current;
    const { minimum, maximum, step } = domain;
    const ceiling = Math.min(maximum, Number.MAX_SAFE_INTEGER);
    const last = minimum + Math.floor((ceiling - minimum) / step) * step;
    if (mode === 'randomize')
        return minimum + Math.floor(random() * (Math.floor((ceiling - minimum) / step) + 1)) * step;
    if (mode === 'increment')
        return current + step > ceiling ? minimum : current + step;
    return current - step < minimum ? last : current - step;
}
