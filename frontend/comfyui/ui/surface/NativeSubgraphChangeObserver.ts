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
/** Observe Comfy's native graph change boundary without owning graph state. */

type NativeAfterChange = (graph: unknown, info?: unknown) => void;
type NativeMutation = (...args: unknown[]) => unknown;

export interface ObservableNativeSubgraph {
  onAfterChange?: NativeAfterChange | null;
  add?: (...args: never[]) => unknown;
  remove?: (...args: never[]) => unknown;
}

/** Chain one presentation notification onto Comfy's authoritative callback. */
export class NativeSubgraphChangeObserver {
  readonly #subgraph: ObservableNativeSubgraph;
  readonly #previous: NativeAfterChange | null;
  readonly #listener: NativeAfterChange;
  readonly #previousAdd: NativeMutation;
  readonly #addListener: NativeMutation;
  readonly #previousRemove: NativeMutation;
  readonly #removeListener: NativeMutation;
  #notificationQueued = false;

  /** Attach without replacing behavior already installed by Comfy. */
  constructor(subgraph: ObservableNativeSubgraph, onChange: () => void) {
    if (typeof subgraph.add !== 'function' || typeof subgraph.remove !== 'function') {
      throw new TypeError('Comfy native subgraph topology methods are unavailable.');
    }
    this.#subgraph = subgraph;
    const notify = (): void => {
      if (this.#notificationQueued) return;
      this.#notificationQueued = true;
      queueMicrotask(() => {
        this.#notificationQueued = false;
        onChange();
      });
    };
    this.#previous = typeof subgraph.onAfterChange === 'function' ? subgraph.onAfterChange : null;
    this.#listener = (graph, info): void => {
      this.#previous?.call(subgraph, graph, info);
      notify();
    };
    this.#previousAdd = subgraph.add as NativeMutation;
    this.#addListener = (...args): unknown => {
      const result = this.#previousAdd.apply(subgraph, args);
      notify();
      return result;
    };
    this.#previousRemove = subgraph.remove as NativeMutation;
    this.#removeListener = (...args): unknown => {
      const result = this.#previousRemove.apply(subgraph, args);
      notify();
      return result;
    };
    subgraph.onAfterChange = this.#listener;
    subgraph.add = this.#addListener;
    subgraph.remove = this.#removeListener;
  }

  /** Restore the callback only while this observer still owns the boundary. */
  dispose(): void {
    if (this.#subgraph.onAfterChange === this.#listener) {
      this.#subgraph.onAfterChange = this.#previous;
    }
    if (this.#subgraph.add === this.#addListener) {
      this.#subgraph.add = this.#previousAdd;
    }
    if (this.#subgraph.remove === this.#removeListener) {
      this.#subgraph.remove = this.#previousRemove;
    }
  }
}
