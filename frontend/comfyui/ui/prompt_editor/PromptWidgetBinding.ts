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
/** Synchronize committed editor source with the original Comfy widget and its graph transaction. */
import type { ComfyNode, ComfyWidget } from '../types/graph.js';
import { isRecord } from '../types/common.js';

/** Observe programmatic widget replacement while preserving native accessors and callback ownership. */
export class PromptWidgetBinding {
  readonly #node: ComfyNode;
  readonly #widget: ComfyWidget;
  readonly #descriptor: PropertyDescriptor | undefined;
  readonly #inheritedAccessor: boolean;
  readonly #changed: (source: string) => void;
  #value: unknown;
  #writing = false;
  #observing = false;

  /** Capture the actual widget once; renderer projections never become value owners. */
  constructor(node: ComfyNode, widget: ComfyWidget, changed: (source: string) => void) {
    this.#node = node;
    this.#widget = widget;
    this.#changed = changed;
    this.#descriptor = Object.getOwnPropertyDescriptor(widget, 'value');
    const original = nativeValueDescriptor(widget);
    this.#inheritedAccessor = !this.#descriptor && !!(original?.get || original?.set);
    this.#value = widget.value;
    if (this.#descriptor?.configurable === false) return;
    Object.defineProperty(widget, 'value', {
      configurable: true,
      enumerable: original?.enumerable ?? true,
      get: () => (original?.get ? original.get.call(widget) : this.#value),
      set: (value: unknown) => {
        if (original?.set) original.set.call(widget, value);
        else {
          if (original?.get || original?.writable === false)
            throw new TypeError('The native prompt value is read-only.');
          this.#value = value;
        }
        const source: unknown = widget.value;
        if (!this.#writing && typeof source === 'string') this.#changed(source);
      },
    });
    this.#observing = true;
  }

  /** Read exact serialized text without coercing invalid host values. */
  read(): string {
    return typeof this.#widget.value === 'string' ? this.#widget.value : '';
  }

  /**
   * Commit one source edit through native callbacks and one balanced workflow history bracket.
   * A value those callbacks write in its place is reported like any external update.
   */
  commit(source: string): void {
    if (this.#writing || source === this.#widget.value) return;
    const node = this.#node;
    const widget = this.#widget;
    const root = isRecord(node.rootGraph) ? node.rootGraph : node.graph;
    const before = root?.beforeChange;
    const after = root?.afterChange;
    const transactional = typeof before === 'function' && typeof after === 'function';
    const previous = widget.value;
    this.#writing = true;
    try {
      if (transactional) before.call(root);
      try {
        widget.value = source;
        widget.callback?.call(widget, source, node, node.graph?.canvas);
        if (typeof node.onWidgetChanged === 'function')
          node.onWidgetChanged.call(node, widget.name, source, previous, widget);
        node.graph?.setDirtyCanvas?.(true, true);
      } finally {
        if (transactional) after.call(root);
      }
    } finally {
      this.#writing = false;
    }
    // A host owner may rewrite the value while this edit's callbacks run, inside the same step.
    const settled: unknown = widget.value;
    if (typeof settled === 'string' && settled !== source) this.#changed(settled);
  }

  /** Restore native property ownership while retaining the last committed text. */
  dispose(): void {
    if (!this.#observing) return;
    this.#observing = false;
    const value: unknown = this.#widget.value;
    if (this.#descriptor) {
      Object.defineProperty(
        this.#widget,
        'value',
        this.#descriptor.get || this.#descriptor.set
          ? this.#descriptor
          : { ...this.#descriptor, value },
      );
    } else {
      Reflect.deleteProperty(this.#widget, 'value');
      if (!this.#inheritedAccessor && this.#widget.value !== value) this.#widget.value = value;
    }
  }
}

/** Locate the host's actual value contract, including accessors implemented on native widget classes. */
function nativeValueDescriptor(widget: object): PropertyDescriptor | undefined {
  let current: object | null = widget;
  while (current) {
    const descriptor = Object.getOwnPropertyDescriptor(current, 'value');
    if (descriptor) return descriptor;
    const prototype: unknown = Object.getPrototypeOf(current);
    current = typeof prototype === 'object' && prototype !== null ? prototype : null;
  }
  return undefined;
}
