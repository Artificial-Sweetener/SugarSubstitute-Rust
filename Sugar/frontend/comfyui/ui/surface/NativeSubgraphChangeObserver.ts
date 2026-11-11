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

/** Distinguish native topology from generic changes before scheduling notifications. */
export type NativeGraphChangeKind = 'change' | 'add' | 'remove';

export interface NativeGraphChange {
  readonly kind: NativeGraphChangeKind;
  readonly info: unknown;
}

export interface ObservableNativeSubgraph {
  onAfterChange?: NativeAfterChange | null;
  add?: (...args: never[]) => unknown;
  remove?: (...args: never[]) => unknown;
  clear?: (...args: never[]) => unknown;
}

type NativeGraphTapListener = (kind: NativeGraphChangeKind, info: unknown) => void;
type TappedMethod = 'onAfterChange' | 'add' | 'remove' | 'clear';

/**
 * Own the single interception of one native graph's change methods.
 *
 * Observers with unrelated lifetimes (runtime, masonry, relations, canvases) observe the same
 * persistent root graph. Chaining one wrapper per observer strands disposed observers, and their
 * closures, whenever they do not dispose in reverse order. One tap per graph keeps every wrapper
 * free of observer state, so a wrapper that cannot be removed retains nothing but the tap.
 */
class NativeGraphTap {
  readonly #graph: ObservableNativeSubgraph;
  readonly #listeners = new Set<NativeGraphTapListener>();
  readonly #wrappers = new Map<TappedMethod, { wrapper: unknown; previous: unknown }>();

  /** Bind one graph without installing wrappers until an observer registers. */
  constructor(graph: ObservableNativeSubgraph) {
    this.#graph = graph;
  }

  /** Add one listener and ensure the current graph methods report to this tap. */
  register(listener: NativeGraphTapListener): () => void {
    this.#listeners.add(listener);
    this.#install();
    return () => {
      if (!this.#listeners.delete(listener) || this.#listeners.size) return;
      this.#restore();
    };
  }

  /** Wrap only methods whose current value is not already this tap's active wrapper. */
  #install(): void {
    const graph = this.#graph;
    if (graph.onAfterChange !== this.#wrappers.get('onAfterChange')?.wrapper) {
      const previous = typeof graph.onAfterChange === 'function' ? graph.onAfterChange : null;
      const wrapper: NativeAfterChange = (changed, info) => {
        previous?.call(graph, changed, info);
        if (this.#current('onAfterChange', wrapper)) this.#notify('change', info);
      };
      this.#wrappers.set('onAfterChange', { wrapper, previous });
      graph.onAfterChange = wrapper;
    }
    for (const method of ['add', 'remove', 'clear'] as const) {
      if (typeof graph[method] !== 'function') continue;
      if (graph[method] === this.#wrappers.get(method)?.wrapper) continue;
      const previous = graph[method] as NativeMutation;
      // Clearing a graph removes every node without calling its per-node removal method.
      const kind = method === 'clear' ? 'remove' : method;
      const wrapper: NativeMutation = (...args) => {
        const result = previous.apply(graph, args);
        if (this.#current(method, wrapper)) this.#notify(kind, method === 'clear' ? null : args[0]);
        return result;
      };
      this.#wrappers.set(method, { wrapper, previous });
      graph[method] = wrapper;
    }
  }

  /** Restore each method still owned by this tap; buried wrappers stay inert pass-throughs. */
  #restore(): void {
    for (const [method, { wrapper, previous }] of this.#wrappers) {
      if (this.#graph[method] !== wrapper) continue;
      Reflect.set(this.#graph, method, previous);
      this.#wrappers.delete(method);
    }
  }

  /** Report only through the newest wrapper so re-wrapping never duplicates a notification. */
  #current(method: TappedMethod, wrapper: unknown): boolean {
    return this.#wrappers.get(method)?.wrapper === wrapper;
  }

  /** Fan one native mutation out to the observers registered at call time. */
  #notify(kind: NativeGraphChangeKind, info: unknown): void {
    for (const listener of [...this.#listeners]) listener(kind, info);
  }
}

const taps = new WeakMap<ObservableNativeSubgraph, NativeGraphTap>();

/** Resolve the one tap owning a graph's native change interception. */
function graphTap(graph: ObservableNativeSubgraph): NativeGraphTap {
  let tap = taps.get(graph);
  if (!tap) {
    tap = new NativeGraphTap(graph);
    taps.set(graph, tap);
  }
  return tap;
}

/** Deliver coalesced native graph changes to one presentation owner. */
export class NativeSubgraphChangeObserver {
  readonly #deliver: () => void;
  readonly #release: () => void;
  #notificationQueued = false;
  readonly #changes: NativeGraphChange[] = [];
  #active = true;

  /** Attach without replacing behavior already installed by Comfy. */
  constructor(
    subgraph: ObservableNativeSubgraph,
    onChange: (changes: readonly NativeGraphChange[]) => void,
    accepts: (kind: NativeGraphChangeKind) => boolean = () => true,
  ) {
    if (typeof subgraph.add !== 'function' || typeof subgraph.remove !== 'function') {
      throw new TypeError('Comfy native subgraph topology methods are unavailable.');
    }
    this.#deliver = () => {
      this.#notificationQueued = false;
      const changes = Object.freeze(this.#changes.splice(0));
      if (this.#active && changes.length) onChange(changes);
    };
    this.#release = graphTap(subgraph).register((kind, info) => {
      if (!this.#active || !accepts(kind)) return;
      if (!this.#changes.some((change) => change.kind === kind && change.info === info))
        this.#changes.push(Object.freeze({ kind, info }));
      if (this.#notificationQueued) return;
      this.#notificationQueued = true;
      queueMicrotask(this.#deliver);
    });
  }

  /** Deliver identified pending mutations before their native transaction commits. */
  flush(): void {
    this.#deliver();
  }

  /** Stop delivery and release this observer from the graph's shared interception. */
  dispose(): void {
    if (!this.#active) return;
    this.#active = false;
    this.#changes.length = 0;
    this.#release();
  }
}
