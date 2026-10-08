//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Measure Fluent advanced destinations without changing native values or persisted disclosure state. */
/** Measure basic or expanded native fields while retaining their current animation constraints. */
export function measureNativeFields(fields, closed) {
    return withNativeFields(fields, closed, () => intrinsicHeight(fields));
}
/** Measure the current projected root after a remount, including its header and moving footer. */
export function measureProjectedAdvancedHeight(target, closed) {
    const root = target.querySelector('.lg-node');
    if (!root)
        return intrinsicHeight(target);
    const fields = [...root.querySelectorAll('[data-sc-fluent-fields]')].find((element) => element.closest('.lg-node') === root);
    const height = root.style.getPropertyValue('height');
    const priority = root.style.getPropertyPriority('height');
    try {
        root.style.setProperty('height', 'auto', 'important');
        return fields
            ? withNativeFields(fields, closed, () => intrinsicHeight(root))
            : intrinsicHeight(root);
    }
    finally {
        root.style.setProperty('height', height, priority);
    }
}
/** Temporarily exclude advanced rows while preserving custom display styles and animation height. */
function withNativeFields(fields, closed, measure) {
    const rows = closed
        ? [...fields.querySelectorAll(':scope > [data-sc-fluent-advanced-row]')]
        : [];
    const styles = rows.map((row) => ({
        value: row.style.getPropertyValue('display'),
        priority: row.style.getPropertyPriority('display'),
    }));
    const height = fields.style.getPropertyValue('height');
    const priority = fields.style.getPropertyPriority('height');
    try {
        fields.style.removeProperty('height');
        for (const row of rows)
            row.style.display = 'none';
        return measure();
    }
    finally {
        fields.style.setProperty('height', height, priority);
        rows.forEach((row, index) => row.style.setProperty('display', styles[index]?.value ?? '', styles[index]?.priority ?? ''));
    }
}
/** Keep measurements in native CSS units so graph camera zoom cannot change the destination. */
function intrinsicHeight(element) {
    const value = Number.parseFloat(element.ownerDocument.defaultView?.getComputedStyle(element).height ?? '');
    return Number.isFinite(value) ? value : element.offsetHeight;
}
