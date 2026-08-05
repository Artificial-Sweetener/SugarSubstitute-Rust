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
/** Measure Nodes 1.0 face bodies without mutating graph-owned slot collections. */
import { isRecord } from '../types/common.js';
import { cubeFaceNodeHasVisibleWidgets, cubeFaceNodeWidgetStartY, } from './CubeFaceNodePresentationPolicy.js';
import { cubeFacePromptWidgetHeight } from './CubeFacePromptTextarea.js';
const HEADER_ONLY_BODY_HEIGHT = 1;
const LEGACY_SLOT_HEIGHT = 20;
const LEGACY_WIDGET_HEIGHT = 20;
const LEGACY_WIDGET_GAP = 4;
const LEGACY_NODE_MARGIN = 6;
/** Measure the native expanded widget body while excluding graph boundary rows. */
export function measureCubeFaceNodeBodyHeight(node) {
    const nativePreviewHeight = hasNativePreviewMedia(node) ? positiveNumber(node.size?.[1]) : null;
    if (!cubeFaceNodeHasVisibleWidgets(node)) {
        return nativePreviewHeight ?? HEADER_ONLY_BODY_HEIGHT;
    }
    const width = positiveNumber(node.size?.[0]) ?? 200;
    let widgetsHeight = cubeFaceNodeWidgetStartY(node);
    for (const widget of visibleWidgets(node)) {
        widgetsHeight += measureWidgetAllocation(widget, node, width);
    }
    const constructorState = Reflect.get(node, 'constructor');
    const slotStart = nonNegativeNumber(readMember(constructorState, 'slot_start_y')) ?? 0;
    const minimumHeight = nonNegativeNumber(readMember(constructorState, 'min_height')) ?? 0;
    const widgetBodyHeight = Math.max(slotStart + LEGACY_SLOT_HEIGHT, widgetsHeight, minimumHeight) + LEGACY_NODE_MARGIN;
    return nativePreviewHeight === null
        ? widgetBodyHeight
        : Math.max(widgetBodyHeight, nativePreviewHeight);
}
/** Detect native card media that must retain Comfy's authored preview allocation. */
function hasNativePreviewMedia(node) {
    const imgs = Reflect.get(node, 'imgs');
    const animatedImages = Reflect.get(node, 'animatedImages');
    return ((Array.isArray(imgs) && imgs.length > 0) ||
        (Array.isArray(animatedImages) && animatedImages.length > 0));
}
/** Return only widgets Comfy currently presents on the real graph node. */
function visibleWidgets(node) {
    const widgets = node.widgets ?? [];
    const isWidgetVisible = node.isWidgetVisible;
    if (typeof isWidgetVisible !== 'function')
        return widgets;
    return widgets.filter((widget) => isWidgetVisible.call(node, widget) !== false);
}
/** Measure the complete vertical allocation Comfy's arrange pass assigns one widget. */
function measureWidgetAllocation(widget, node, width) {
    const promptHeight = cubeFacePromptWidgetHeight(widget);
    if (promptHeight !== null)
        return promptHeight;
    const computedHeight = positiveNumber(widget.computedHeight);
    if (computedHeight !== null)
        return computedHeight;
    if (typeof widget.computeSize === 'function') {
        const size = widget.computeSize(width);
        if (Array.isArray(size)) {
            return (positiveNumber(size[1]) ?? LEGACY_WIDGET_HEIGHT) + LEGACY_WIDGET_GAP;
        }
    }
    if (typeof widget.computeLayoutSize === 'function') {
        const size = widget.computeLayoutSize(node);
        if (isRecord(size))
            return positiveNumber(size.minHeight) ?? LEGACY_WIDGET_HEIGHT;
    }
    return LEGACY_WIDGET_HEIGHT + LEGACY_WIDGET_GAP;
}
/** Narrow one dynamic positive dimension. */
function positiveNumber(value) {
    return typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : null;
}
/** Narrow one dynamic non-negative dimension. */
function nonNegativeNumber(value) {
    return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : null;
}
/** Read one dynamic object or constructor member. */
function readMember(value, key) {
    if (typeof value === 'function' || isRecord(value))
        return Reflect.get(value, key);
    return undefined;
}
