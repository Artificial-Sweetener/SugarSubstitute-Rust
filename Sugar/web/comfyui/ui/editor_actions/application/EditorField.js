import { copySavablePresetValue } from '../domain/PresetCodec.js';
/** Capture detached editable JSON inputs without persisting execution controls or connections. */
export function captureEditorInputs(fields) {
    return Object.fromEntries(fields.flatMap((field) => {
        if (field.disabled())
            return [];
        const value = copySavablePresetValue(field.read());
        if (value === undefined)
            return [];
        return [[field.name, value]];
    }));
}
/** Apply matching inputs through host owners while reporting skipped fields for diagnostics. */
export function applyEditorInputs(fields, inputs) {
    const skipped = [];
    for (const [name, value] of Object.entries(inputs)) {
        const field = fields.find((field) => field.name === name);
        const detached = copySavablePresetValue(value);
        if (!field || detached === undefined || !field.write(detached))
            skipped.push(name);
    }
    return skipped;
}
