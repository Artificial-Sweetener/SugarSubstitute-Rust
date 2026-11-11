import { isRecord } from '../../types/common.js';
/** Admit serialized configurable values while excluding execution accessories and buttons. */
export function nativeFieldDomain(node, widget, inputName = widget.name) {
    if (/^unknown(?:_\d+)?$/.test(widget.name) || widget.name === 'control_after_generate')
        return null;
    if (widget.serialize === false ||
        widget.name === 'control_after_generate' ||
        !isFieldValue(widget.value))
        return null;
    const constructor = Reflect.get(node, 'constructor');
    const definition = typeof constructor === 'function' ? Reflect.get(constructor, 'nodeData') : null;
    const input = isRecord(definition) && isRecord(definition.input) ? definition.input : {};
    const required = isRecord(input.required) ? input.required : {}, optional = isRecord(input.optional) ? input.optional : {};
    const schema = required[inputName] ?? optional[inputName];
    const type = Array.isArray(schema) ? schema[0] : null;
    const spec = Array.isArray(schema) && isRecord(schema[1])
        ? schema[1]
        : isRecord(widget.spec)
            ? widget.spec
            : {};
    const options = widget.options ?? {};
    const values = typeof options.values === 'function' ? options.values.call(widget, node) : options.values;
    if (Array.isArray(values) && values.every((value) => typeof value === 'string'))
        return values.length ? { kind: 'choice', values: [...values] } : null;
    if (Array.isArray(type) && type.every((value) => typeof value === 'string'))
        return type.length ? { kind: 'choice', values: [...type] } : null;
    const widgetType = widget.type?.toLowerCase();
    if (type === 'INT' || type === 'FLOAT' || widgetType === 'number' || widgetType === 'slider') {
        const integer = type === 'INT' || spec.type === 'INT' || options.precision === 0;
        const minimum = finite(spec.min) ?? finite(options.min) ?? -Number.MAX_SAFE_INTEGER;
        const maximum = finite(spec.max) ?? finite(options.max) ?? Number.MAX_SAFE_INTEGER;
        const step = finite(spec.step) ??
            finite(options.step2) ??
            (integer ? 1 : finite(options.step) !== null ? Number(options.step) / 10 : 0.01);
        return maximum >= minimum && step > 0
            ? { kind: 'number', minimum, maximum, step: integer ? Math.max(1, step) : step, integer }
            : null;
    }
    if (type === 'BOOLEAN' || widgetType === 'toggle' || widgetType === 'boolean')
        return { kind: 'boolean' };
    if (type === 'STRING' ||
        widgetType === 'text' ||
        widgetType === 'customtext' ||
        widgetType === 'string')
        return { kind: 'string' };
    return null;
}
/** Keep dynamic or structured widget values outside configurable relation state. */
export function isFieldValue(value) {
    return (typeof value === 'string' ||
        typeof value === 'boolean' ||
        (typeof value === 'number' && Number.isFinite(value)));
}
/** Narrow numeric host constraints before creating a domain. */
function finite(value) {
    return typeof value === 'number' && Number.isFinite(value) ? value : null;
}
