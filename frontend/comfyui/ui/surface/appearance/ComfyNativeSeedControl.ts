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
/** Resolve Comfy's authoritative seed mode through native and promoted widget boundaries. */
import { isRecord } from '../../types/common.js';
import type { UnknownRecord } from '../../types/common.js';

import { isSeedMode, isComfySeedModeControl } from '../../graph/ComfySeedMode.js';
import type { SeedMode } from '../../graph/ComfySeedMode.js';

export interface NativeSeedControl {
  readonly modes: readonly SeedMode[];
  readMode(): SeedMode;
  writeMode(mode: SeedMode): void;
}

const seedNames = new Set(['seed', 'noise_seed', 'variation_seed']);

/** Resolve native numeric owners by scoped locator, retaining subgraph identity. */
export class ComfyNativeSeedControlAdapter {
  readonly #app: unknown;
  readonly #nodes = new Map<string, UnknownRecord>();

  /** Bind the application graph boundary without relying on browser globals. */
  constructor(app: unknown) {
    this.#app = app;
  }

  /** Index each live graph once per coalesced appearance batch. */
  beginBatch(): void {
    this.#nodes.clear();
    const app = isRecord(this.#app) ? this.#app : {};
    const seen = new Set<object>();
    const visit = (value: unknown): void => {
      if (!isRecord(value) || seen.has(value)) return;
      seen.add(value);
      if (Array.isArray(value._nodes)) {
        for (const node of value._nodes) {
          if (!isRecord(node) || (typeof node.id !== 'string' && typeof node.id !== 'number'))
            continue;
          this.#nodes.set(`${String(value.id)}:${String(node.id)}`, node);
          if (value === app.graph) this.#nodes.set(String(node.id), node);
          visit(node.subgraph);
        }
      }
      if (value._subgraphs instanceof Map)
        for (const graph of value._subgraphs.values()) visit(graph);
      if (isRecord(value.rootGraph)) visit(value.rootGraph);
    };
    visit(app.graph);
  }

  /** Release graph references immediately after native discovery. */
  endBatch(): void {
    this.#nodes.clear();
  }

  /** Pair only integer editors with a validated host seed-mode control. */
  read(widget: unknown, precision: number | null): NativeSeedControl | null {
    if (!isRecord(widget) || typeof widget.name !== 'string') return null;
    const spec = isRecord(widget.spec) ? widget.spec : {};
    if (precision !== 0 && spec.type !== 'INT') return null;
    if (spec.control_after_generate === false) return null;
    const locator = widget.nodeLocatorId;
    const node = typeof locator === 'string' ? this.#nodes.get(locator) : undefined;
    if (!node) return null;
    return readNativeSeedControl(node, widget.name, spec.control_after_generate === true);
  }
}

/** Share native promoted seed ownership with execution controls and relation coordination. */
export function readNativeSeedControl(
  node: UnknownRecord,
  name: string,
  allowFallback = false,
): NativeSeedControl | null {
  const owner = resolveOwner(node, name, new Set());
  if (!owner) return null;
  const linked = owner.widget.linkedWidgets;
  const candidates = Array.isArray(linked) ? linked : [];
  let control = candidates.find(isComfySeedModeControl);
  if (!control && (seedNames.has(name) || allowFallback)) {
    const widgets = owner.node.widgets;
    if (Array.isArray(widgets)) {
      const index = widgets.indexOf(owner.widget);
      const next = widgets[index + 1];
      if (isRecord(next) && next.name === 'control_after_generate' && isComfySeedModeControl(next))
        control = next;
    }
  }
  if (!isComfySeedModeControl(control)) return null;
  const options = isRecord(control.options) ? control.options : {};
  const modes = Array.isArray(options.values) ? options.values.filter(isSeedMode) : [];
  const nativeControl = control;
  return {
    modes,
    readMode: () => (isSeedMode(nativeControl.value) ? nativeControl.value : 'fixed'),
    writeMode: (mode) => {
      if (!modes.includes(mode) || nativeControl.value === mode) return;
      const previous = nativeControl.value;
      nativeControl.value = mode;
      if (typeof nativeControl.callback === 'function')
        nativeControl.callback.call(nativeControl, mode);
      if (typeof owner.node.onWidgetChanged === 'function')
        owner.node.onWidgetChanged.call(
          owner.node,
          nativeControl.name,
          mode,
          previous,
          nativeControl,
        );
      const graph = isRecord(owner.node.graph) ? owner.node.graph : {};
      if (typeof graph.afterChange === 'function') graph.afterChange.call(graph);
    },
  };
}

/** Follow exposed subgraph inputs to the original widget and its linked execution control. */
function resolveOwner(
  node: UnknownRecord,
  name: string,
  seen: Set<object>,
): { node: UnknownRecord; widget: UnknownRecord } | null {
  if (seen.has(node)) return null;
  seen.add(node);
  const widget = Array.isArray(node.widgets)
    ? node.widgets.find((value: unknown) => isRecord(value) && value.name === name)
    : undefined;
  if (!isRecord(widget)) return null;
  if (Array.isArray(widget.linkedWidgets) && widget.linkedWidgets.some(isComfySeedModeControl))
    return { node, widget };
  const graph = isRecord(node.subgraph) ? node.subgraph : {};
  const inputNode = isRecord(graph.inputNode) ? graph.inputNode : {};
  const slot = Array.isArray(inputNode.slots)
    ? inputNode.slots.find((value: unknown) => isRecord(value) && value.name === name)
    : undefined;
  if (isRecord(slot) && Array.isArray(slot.linkIds) && typeof graph.getLink === 'function') {
    for (const id of slot.linkIds) {
      const link: unknown = graph.getLink.call(graph, id);
      if (!isRecord(link) || !Array.isArray(graph._nodes)) continue;
      const target = graph._nodes.find(
        (value: unknown) => isRecord(value) && String(value.id) === String(link.target_id),
      );
      if (
        !isRecord(target) ||
        !Array.isArray(target.inputs) ||
        typeof link.target_slot !== 'number'
      )
        continue;
      const input: unknown = target.inputs[link.target_slot];
      const binding = isRecord(input) && isRecord(input.widget) ? input.widget : {};
      if (typeof binding.name !== 'string') continue;
      const resolved = resolveOwner(target, binding.name, seen);
      if (resolved) return resolved;
    }
  }
  return { node, widget };
}
