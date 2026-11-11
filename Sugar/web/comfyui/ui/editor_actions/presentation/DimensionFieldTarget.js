import { inferDimensionPairs } from '../domain/DimensionPolicy.js';
/** Include each field's label and padding, using the nearest column only within a real shared row. */
export function dimensionFieldTarget(root, fields, target, x, y) {
    const direct = fields.find((field) => field.element !== root && field.element.contains(target));
    if (direct)
        return direct;
    for (const pair of inferDimensionPairs(fields.map((field) => field.name))) {
        const width = fields.find((field) => field.name === pair.width);
        const height = fields.find((field) => field.name === pair.height);
        if (!width ||
            !height ||
            width.kind !== 'number' ||
            height.kind !== 'number' ||
            width.element === root ||
            height.element === root ||
            width.element.parentElement !== target ||
            height.element.parentElement !== target)
            continue;
        const first = width.element.getBoundingClientRect();
        const second = height.element.getBoundingClientRect();
        const top = Math.max(first.top, second.top), bottom = Math.min(first.bottom, second.bottom);
        if (first.width <= 0 ||
            second.width <= 0 ||
            bottom <= top ||
            y < top ||
            y > bottom ||
            x < Math.min(first.left, second.left) ||
            x > Math.max(first.right, second.right))
            continue;
        const widthDistance = Math.abs(x - (first.left + first.right) / 2);
        const heightDistance = Math.abs(x - (second.left + second.right) / 2);
        return widthDistance <= heightDistance ? width : height;
    }
    return null;
}
