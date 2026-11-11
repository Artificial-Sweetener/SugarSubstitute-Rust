import { createSeedActionIcon } from './SeedActionIcons.js';
/** Name native generation modes consistently across both seed menu presentations. */
export const modeLabels = {
    fixed: 'Use Current',
    randomize: 'Randomize',
    increment: 'Increment',
    decrement: 'Decrement',
};
/** Present exactly the supported modes followed by the last-used seed action. */
export function nativeSeedMenuEntries(control, choose, reuse, fluent = true) {
    return [
        ...control.modes.map((mode) => ({
            label: modeLabels[mode],
            checked: control.readMode() === mode,
            attributes: { 'data-sc-seed-option': mode },
            icon: (document) => createSeedActionIcon(document, mode, fluent),
            select: () => choose(mode),
        })),
        {
            label: 'Reuse last',
            disabled: !control.reuse || control.reuse.readLast() === null,
            attributes: { 'data-sc-seed-option': 'reuse-last' },
            icon: (document) => createSeedActionIcon(document, 'reuse-last', fluent),
            select: reuse,
        },
    ];
}
