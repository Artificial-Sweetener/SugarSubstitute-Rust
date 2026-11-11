import { inferDimensionPairs } from '../domain/DimensionPolicy.js';
import { GLOBAL_ASSOCIATION } from '../domain/UserPreset.js';
import { captureEditorInputs, applyEditorInputs } from '../application/EditorField.js';
import { dimensionMenuEntries } from './DimensionMenu.js';
/** Consume prepared libraries and explicit command collaborators instead of owning persistence. */
export function nodeActionEntries(card, library, families, dialog, run, skipped, modelContributions = [], canvasDimensions) {
    const scopes = [...families, GLOBAL_ASSOCIATION];
    const presets = library.nodePresets(card.nodeType, scopes);
    const entries = [];
    if (presets.length) {
        const seen = new Set();
        const populated = scopes.flatMap((scope) => {
            const selected = presets.filter((preset) => !seen.has(preset.id) &&
                preset.associations.some((association) => association.scope === scope.scope &&
                    association.provider === scope.provider &&
                    association.key === scope.key));
            for (const preset of selected)
                seen.add(preset.id);
            return selected.length ? [{ scope, presets: selected }] : [];
        });
        entries.push({
            label: 'Apply preset',
            children: populated.flatMap(({ scope, presets }, index) => [
                ...(index > 0 ? [{ label: '', separator: true }] : []),
                ...(populated.length > 1 ? [{ label: scope.label, section: true }] : []),
                ...presets.map((preset) => ({
                    label: preset.label,
                    attributes: {
                        title: `${card.nodeType} - ${Object.keys(preset.payload.inputs).length} inputs`,
                    },
                    select: () => {
                        const names = applyEditorInputs(card.fields, preset.payload.inputs);
                        if (names.length)
                            skipped(names);
                    },
                })),
            ]),
        });
    }
    const inputs = captureEditorInputs(card.fields);
    if (Object.keys(inputs).length) {
        if (entries.length)
            entries.push({ label: '', separator: true });
        entries.push({
            label: `Save current ${card.title} as preset...`,
            select: () => run((async () => {
                const result = await dialog.open(`Save ${card.title} preset`, [
                    GLOBAL_ASSOCIATION,
                    ...families,
                ]);
                if (result)
                    await library.saveNode(result.name, card.nodeType, inputs, result.association);
            })()),
        });
    }
    const fieldNames = card.fields.map((field) => field.name);
    const contributions = modelContributions.map((contribution) => ({
        order: fieldNames.indexOf(contribution.fieldName),
        entries: contribution.entries,
    }));
    for (const pair of inferDimensionPairs(fieldNames)) {
        const width = card.fields.find((field) => field.name === pair.width), height = card.fields.find((field) => field.name === pair.height);
        if (!width || !height || width.kind !== 'number' || height.kind !== 'number')
            continue;
        const binding = { width, height };
        const dimensions = dimensionMenuEntries(binding, null, library, families, run, canvasDimensions?.(binding));
        contributions.push({
            order: Math.min(fieldNames.indexOf(pair.width), fieldNames.indexOf(pair.height)),
            entries: dimensions,
        });
    }
    for (const contribution of contributions.sort((left, right) => left.order - right.order)) {
        if (!contribution.entries.length)
            continue;
        if (entries.length)
            entries.push({ label: '', separator: true });
        entries.push(...contribution.entries);
    }
    if (card.advanced) {
        if (entries.length)
            entries.push({ label: '', separator: true });
        entries.push({
            label: 'Show advanced inputs',
            checked: card.advanced.shown(),
            selection: 'multiple',
            select: () => card.advanced?.setShown(!card.advanced.shown()),
        });
    }
    return entries;
}
