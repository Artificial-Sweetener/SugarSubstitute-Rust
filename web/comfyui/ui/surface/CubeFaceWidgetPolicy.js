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
/** Own effective widget visibility for projected Cube-face node cards. */
import { isNativeWidgetPresented } from './NativeWidgetPresentation.js';
/** Resolve rendered fields while retaining their underlying editing capabilities for discovery. */
export function cubeFacePresentedWidgets(node) {
    return cubeFaceVisibleWidgets(node).filter(isNativeWidgetPresented);
}
/** Resolve widget eligibility independently from the card's collapsed presentation. */
export function cubeFaceVisibleWidgets(node) {
    const isWidgetVisible = node.isWidgetVisible;
    const flags = node.flags;
    const ownedFlags = Object.prototype.hasOwnProperty.call(node, 'flags');
    // Comfy folds whole-card collapse into widget visibility. Query the expanded
    // capability synchronously, retaining native hidden/advanced/custom rules.
    if (flags?.collapsed === true)
        node.flags = { ...flags, collapsed: false };
    try {
        return (node.widgets ?? []).filter((widget) => !isCubeFaceWidgetConsumed(node, widget) &&
            (typeof isWidgetVisible !== 'function' || isWidgetVisible.call(node, widget) !== false));
    }
    finally {
        if (flags?.collapsed === true) {
            if (ownedFlags)
                node.flags = flags;
            else
                delete node.flags;
        }
    }
}
/** Query content available through disclosure while preserving native hidden and connection rules. */
export function cubeFaceRevealableWidgets(node) {
    const owned = Object.prototype.hasOwnProperty.call(node, 'showAdvanced');
    const previous = node.showAdvanced;
    node.showAdvanced = true;
    try {
        return cubeFaceVisibleWidgets(node);
    }
    finally {
        if (owned)
            node.showAdvanced = previous;
        else
            delete node.showAdvanced;
    }
}
/** Return widgets whose values remain owned by the node regardless of host visibility. */
export function cubeFaceUnconsumedWidgets(node) {
    return (node.widgets ?? []).filter((widget) => !isCubeFaceWidgetConsumed(node, widget));
}
/** Return whether an incoming graph connection owns one widget's effective value. */
export function isCubeFaceWidgetConsumed(node, widget) {
    const widgetName = widget.name.trim();
    if (!widgetName)
        return false;
    return (node.inputs ?? []).some((input) => inputWidgetName(input) === widgetName && inputHasConnection(input));
}
/** Read the stable widget identity carried beside a converted input. */
function inputWidgetName(input) {
    return typeof input.widget?.name === 'string' ? input.widget.name.trim() : '';
}
/** Recognize both singular and multi-link host input representations. */
function inputHasConnection(input) {
    return input.link !== null && input.link !== undefined ? true : (input.links?.length ?? 0) > 0;
}
