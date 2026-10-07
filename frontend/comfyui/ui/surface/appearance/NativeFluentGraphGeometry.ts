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
/** Reconcile native socket caches after a graph card first receives Fluent layout styling. */
import { loadComfyNativeSlotLayoutSync } from '../ComfyRuntimeModuleLoader.js';

export interface NativeFluentGraphGeometryOptions {
  document: Document;
  getSynchronizer?(): Promise<() => void>;
  reportError(error: unknown): void;
}

/** Coalesce initial graph styling without rebuilding inventory or responding to popup activity. */
export class NativeFluentGraphGeometry {
  readonly #options: NativeFluentGraphGeometryOptions;
  readonly #getSynchronizer: () => Promise<() => void>;
  #synchronizer: Promise<() => void> | null = null;
  #styled = new WeakSet<HTMLElement>();
  #frame: number | null = null;
  #generation = 0;
  #pendingGeneration: number | null = null;
  #hasStyles = false;

  /** Resolve the native layout capability without loading another node renderer. */
  constructor(options: NativeFluentGraphGeometryOptions) {
    this.#options = options;
    this.#getSynchronizer =
      options.getSynchronizer ?? (() => loadComfyNativeSlotLayoutSync(options.document));
  }

  /** Request native synchronization once for each newly styled canonical graph card. */
  request(root: HTMLElement): void {
    if (
      !root.isConnected ||
      !root.matches('.lg-node[data-node-id]') ||
      root.closest('.sugarcubes-cube-face__node-card, .sugarcubes-workflow-masonry') ||
      this.#styled.has(root)
    )
      return;
    this.#styled.add(root);
    this.#hasStyles = true;
    this.#queue();
  }

  /** Reconcile released appearance geometry while discarding obsolete styling requests. */
  stylesRemoved(): void {
    const hadStyles = this.#hasStyles;
    this.clear();
    if (hadStyles) this.#queue();
  }

  /** Coalesce the host's graph-wide native hook at a named layout lifecycle boundary. */
  #queue(): void {
    if (this.#frame !== null || this.#pendingGeneration === this.#generation) return;
    const generation = this.#generation;
    this.#frame =
      this.#options.document.defaultView?.requestAnimationFrame(() => {
        this.#frame = null;
        void this.#synchronize(generation);
      }) ?? null;
    if (this.#frame === null) void this.#synchronize(generation);
  }

  /** Cancel retired appearance requests and allow re-enabled styling to synchronize again. */
  clear(): void {
    this.#generation += 1;
    if (this.#frame !== null) this.#options.document.defaultView?.cancelAnimationFrame(this.#frame);
    this.#frame = null;
    this.#pendingGeneration = null;
    this.#styled = new WeakSet();
    this.#hasStyles = false;
    this.#synchronizer = null;
  }

  /** Ask the authoritative native layout owner after styling has had a browser frame to settle. */
  async #synchronize(generation: number): Promise<void> {
    this.#pendingGeneration = generation;
    try {
      this.#synchronizer ??= this.#getSynchronizer();
      const synchronize = await this.#synchronizer;
      if (generation === this.#generation) synchronize();
    } catch (error: unknown) {
      if (generation === this.#generation) this.#options.reportError(error);
    } finally {
      if (this.#pendingGeneration === generation) this.#pendingGeneration = null;
    }
  }
}
