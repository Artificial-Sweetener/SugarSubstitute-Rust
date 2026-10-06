const standardComponents = {
    WidgetInputNumber: 'scalar',
    WidgetInputNumberInput: 'scalar',
    WidgetInputNumberSlider: 'scalar',
    WidgetToggleSwitch: 'scalar',
    WidgetSelect: 'scalar',
    WidgetInputText: 'scalar',
    WidgetTextarea: 'multiline',
};
/** Enumerate the core components whose field layout this adapter understands. */
export const NATIVE_FIELD_COMPONENTS = Object.keys(standardComponents);
/** Accept known native components rather than guessing from arbitrary input markup. */
export function classifyNativeField(field) {
    return standardComponents[field.component] ?? null;
}
/** Group adjacent native fields while preserving authored order and custom boundaries. */
export function planNativeFields(fields) {
    const pairs = new Set(['width:height', 'sampler_name:scheduler', 'steps:cfg']);
    const result = [];
    let row = 1;
    for (let index = 0; index < fields.length; index += 1) {
        const field = fields[index];
        if (!field)
            continue;
        const kind = classifyNativeField(field);
        const next = fields[index + 1];
        const paired = kind === 'scalar' &&
            next &&
            classifyNativeField(next) === 'scalar' &&
            pairs.has(`${field.name}:${next.name}`);
        if (kind)
            result.push({ name: field.name, kind, row, column: 1, span: paired ? 1 : 2 });
        if (paired) {
            result.push({ name: next.name, kind: 'scalar', row, column: 2, span: 1 });
            index += 1;
        }
        row += 1;
    }
    return result;
}
