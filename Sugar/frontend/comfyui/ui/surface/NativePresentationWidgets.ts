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
/** Scope native presentation widgets to projected cards without adding graph fields. */
import type { ComfyNode, ComfyWidget } from '../types/graph.js';

const additions = new WeakMap<ComfyNode, Map<ComfyWidget, string | null>>();

/** Retain a presentation widget with optional placement before an existing graph field. */
export function registerNativePresentationWidget(
  node: ComfyNode,
  widget: ComfyWidget,
  beforeField: string | null = null,
): () => void {
  let widgets = additions.get(node);
  if (!widgets) {
    widgets = new Map();
    additions.set(node, widgets);
  }
  widgets.set(widget, beforeField);
  return () => {
    widgets.delete(widget);
    if (!widgets.size) additions.delete(node);
  };
}

/** Read actual graph fields even while a synchronous renderer temporarily borrows presentation additions. */
export function nativeGraphWidgets(node: ComfyNode): ComfyWidget[] {
  const presentation = additions.get(node);
  return (node.widgets ?? []).filter((widget) => !presentation?.has(widget));
}

/** Resolve presentation additions before their optional native field anchor without changing graph order. */
export function nativePresentationWidgets(node: ComfyNode): ComfyWidget[] {
  const widgets = [...new Set(nativeGraphWidgets(node))];
  for (const [widget, beforeField] of additions.get(node) ?? []) {
    const index = beforeField ? widgets.findIndex((item) => item.name === beforeField) : -1;
    if (index < 0) widgets.push(widget);
    else widgets.splice(index, 0, widget);
  }
  return widgets;
}

/** Expose additions synchronously to Comfy's extractor, then restore the exact live widget list. */
export function withNativePresentationWidgets<Result>(
  node: ComfyNode,
  operation: () => Result,
): Result {
  if (!additions.get(node)?.size) return operation();
  const owned = Object.hasOwn(node, 'widgets'),
    previous = node.widgets,
    items = previous ? [...previous] : null;
  node.widgets = nativePresentationWidgets(node);
  try {
    return operation();
  } finally {
    // Comfy's reactive widget setter may splice the original array instead of replacing it.
    if (previous && items) previous.splice(0, previous.length, ...items);
    if (previous) node.widgets = previous;
    else delete node.widgets;
    if (!owned) delete node.widgets;
  }
}
