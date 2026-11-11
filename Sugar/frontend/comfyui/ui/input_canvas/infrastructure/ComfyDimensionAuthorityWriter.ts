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
/** Write a mask-only canvas's new size into the live width and height fields that size it. */
import type { ComfyGraph, ComfyNode, ComfyWidget } from '../../types/graph.js';
import type { DimensionAuthorityNode } from '../domain/CanvasDimensionAuthority.js';
import type { DimensionAuthorityWriter } from '../application/WorkflowCanvasResolution.js';

/**
 * Commit every authority's fields as one native history step, through each widget's own
 * callback and the node's change notification, as a user edit would.
 */
export class ComfyDimensionAuthorityWriter implements DimensionAuthorityWriter {
  /** Resolve flattened paths through the association inventory and history through the root graph. */
  constructor(
    private readonly node: (path: string) => ComfyNode | null,
    private readonly root: () => ComfyGraph | null | undefined,
  ) {}

  /** Refuse before changing anything when any authority field is gone. */
  write(nodes: readonly DimensionAuthorityNode[], width: number, height: number): void {
    const fields = nodes.flatMap((item) => {
      const node = this.node(item.node);
      const widthField = node?.widgets?.find((widget) => widget.name === item.width);
      const heightField = node?.widgets?.find((widget) => widget.name === item.height);
      if (!node || !widthField || !heightField)
        throw new Error('A node that sizes this canvas is no longer in the workflow.');
      return [
        [node, widthField, width],
        [node, heightField, height],
      ] as const;
    });
    const graph = this.root();
    graph?.beforeChange?.();
    try {
      for (const [node, widget, value] of fields) commit(node, widget, value);
    } finally {
      graph?.afterChange?.();
    }
  }
}

/** Apply one value through the widget's callback and the node's widget-change hook. */
function commit(node: ComfyNode, widget: ComfyWidget, value: number): void {
  const previous = widget.value;
  if (previous === value) return;
  widget.value = value;
  widget.callback?.(value);
  if (typeof node.onWidgetChanged === 'function')
    node.onWidgetChanged.call(node, widget.name, value, previous, widget);
}
