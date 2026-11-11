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
/** Resolve native instance paths and commit seed restoration through host history and refresh boundaries. */
import { isRecord, type UnknownRecord } from '../../types/common.js';
import type { ComfyGraph } from '../../types/graph.js';
import {
  readNativeSeedControl,
  resolveNativeSeedOwner,
} from '../../surface/appearance/ComfyNativeSeedControl.js';
import { refreshNativeNodeCards } from '../../surface/NativeNodeCardRefresh.js';

/** Resolve one owner's full instance path, refusing nodes removed or replaced by native history. */
export function nativeSeedPath(
  graph: unknown,
  target: UnknownRecord,
  compiled = false,
  prefix: readonly string[] = [],
  ancestors: ReadonlySet<object> = new Set(),
): string | null {
  if (
    !isRecord(graph) ||
    !Array.isArray(graph._nodes) ||
    ancestors.has(graph) ||
    prefix.length > 32
  )
    return null;
  const visited = new Set([...ancestors, graph]);
  for (const node of graph._nodes) {
    if (!isRecord(node) || (typeof node.id !== 'string' && typeof node.id !== 'number')) continue;
    const properties = isRecord(node.properties) ? node.properties : {};
    const identity = isRecord(properties.sugarcubes_cube) ? properties.sugarcubes_cube : {};
    const symbol = prefix.length ? properties.sugarcubes_symbol : identity.instance_id;
    const segment =
      compiled && typeof symbol === 'string' && symbol.trim() ? symbol.trim() : String(node.id);
    const path = [...prefix, segment];
    if (node === target) return path.join(':');
    const nested = nativeSeedPath(node.subgraph, target, compiled, path, visited);
    if (nested) return nested;
  }
  return null;
}

/** Restore the original seed and its facade, then publish one history entry and scoped presentation refresh. */
export function restoreNativeSeed(
  graph: ComfyGraph | null | undefined,
  document: Document,
  node: UnknownRecord,
  name: string,
  value: number,
): void {
  const owner = resolveNativeSeedOwner(node, name);
  if (!owner) return;
  graph?.beforeChange?.();
  try {
    writeSeedValue(owner.node, owner.widget, value);
    const facade = Array.isArray(node.widgets)
      ? node.widgets.find((widget: unknown) => isRecord(widget) && widget.name === name)
      : null;
    if (isRecord(facade) && facade !== owner.widget) writeSeedValue(node, facade, value);
    readNativeSeedControl(node, name, true)?.writeMode('fixed', false);
  } finally {
    graph?.afterChange?.();
  }
  graph?.setDirtyCanvas?.(true, true);
  refreshNativeNodeCards(document, { kind: 'nodes', nodes: [...new Set([node, owner.node])] });
}

/** Preserve native value callbacks and change semantics without creating a second history bracket. */
function writeSeedValue(node: UnknownRecord, widget: UnknownRecord, value: number): void {
  const previous = widget.value;
  widget.value = value;
  if (typeof widget.callback === 'function') widget.callback.call(widget, value, node);
  if (typeof node.onWidgetChanged === 'function')
    node.onWidgetChanged.call(node, widget.name, value, previous, widget);
}
