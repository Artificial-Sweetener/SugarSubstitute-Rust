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
/** Isolate the current Comfy synchronous node-creation compatibility seam. */

import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import { isRecord } from '../types/common.js';
import type { UnknownRecord, Vec2 } from '../types/common.js';
import { isComfyCubePickerType } from './ComfyCubeNodeDefProjector.js';
import type { CubePickerPlacementAdapter } from './CubePickerPlacementAdapter.js';

const INSTALLATION_MARKER = Symbol.for('SugarCubes.ComfyCubePickerCreationAdapter');

type HostCreateNode = (type: string, title?: string, options?: UnknownRecord) => unknown;

interface SugarCreationWrapperState {
  namespace: 'SugarCubes.Cube';
  owner: object;
  placement: CubePickerPlacementAdapter;
  previous: HostCreateNode;
  reportError(summary: string, detail: string): void;
}

export interface CubePickerLiteGraphHost extends UnknownRecord {
  createNode: HostCreateNode;
}

export interface ComfyCubePickerCreationAdapterOptions {
  getLiteGraph(): CubePickerLiteGraphHost | null;
  placement: CubePickerPlacementAdapter;
  reportError(summary: string, detail: string): void;
}

/** Claim only Sugar's reserved picker namespace at LiteGraph's shared factory boundary. */
export class ComfyCubePickerCreationAdapter {
  readonly #getLiteGraph: () => CubePickerLiteGraphHost | null;
  readonly #placement: CubePickerPlacementAdapter;
  readonly #reportError: (summary: string, detail: string) => void;
  readonly #owner = {};
  #installedWrapper: HostCreateNode | null = null;
  #previousCreateNode: HostCreateNode | null = null;
  #installedHost: CubePickerLiteGraphHost | null = null;

  /** Bind the compatibility host seam to detached Cube placement semantics. */
  constructor(options: ComfyCubePickerCreationAdapterOptions) {
    this.#getLiteGraph = options.getLiteGraph;
    this.#placement = options.placement;
    this.#reportError = options.reportError;
  }

  /** Install one idempotent, chain-preserving wrapper around the host factory. */
  install(): void {
    const liteGraph = this.#getLiteGraph();
    if (!liteGraph) throw new Error('LiteGraph node creation is unavailable.');
    const current = liteGraph.createNode;
    const existingState = readWrapperState(Reflect.get(current, INSTALLATION_MARKER));
    if (existingState) {
      existingState.owner = this.#owner;
      existingState.placement = this.#placement;
      existingState.reportError = this.#reportError;
      this.#previousCreateNode = existingState.previous;
      this.#installedWrapper = current;
      this.#installedHost = liteGraph;
      return;
    }
    const state: SugarCreationWrapperState = {
      namespace: 'SugarCubes.Cube',
      owner: this.#owner,
      placement: this.#placement,
      previous: current,
      reportError: this.#reportError,
    };
    const wrapper: HostCreateNode = function (
      this: unknown,
      ...args: [type: string, title?: string, options?: UnknownRecord]
    ): unknown {
      const [type, title, options] = args;
      if (!isComfyCubePickerType(type)) {
        return Reflect.apply(current, this, args);
      }
      try {
        const node = state.placement.create(type, readPosition(options?.pos));
        applyHostCreationOptions(node, title, options);
        return node;
      } catch (error: unknown) {
        const detail = readErrorMessage(error);
        state.reportError('SugarCube placement failed', detail);
        throw error;
      }
    };
    Reflect.set(wrapper, INSTALLATION_MARKER, state);
    this.#previousCreateNode = current;
    this.#installedWrapper = wrapper;
    this.#installedHost = liteGraph;
    liteGraph.createNode = wrapper;
  }

  /** Restore the captured host function only when this adapter still owns the seam. */
  dispose(): void {
    const state = this.#installedWrapper
      ? readWrapperState(Reflect.get(this.#installedWrapper, INSTALLATION_MARKER))
      : null;
    if (
      this.#installedWrapper &&
      this.#previousCreateNode &&
      this.#installedHost?.createNode === this.#installedWrapper &&
      state?.owner === this.#owner
    ) {
      this.#installedHost.createNode = this.#previousCreateNode;
    }
    this.#installedWrapper = null;
    this.#previousCreateNode = null;
    this.#installedHost = null;
  }
}

/** Read the shared wrapper state retained across extension module reloads. */
function readWrapperState(value: unknown): SugarCreationWrapperState | null {
  if (
    !isRecord(value) ||
    value.namespace !== 'SugarCubes.Cube' ||
    typeof value.placement !== 'object' ||
    value.placement === null ||
    typeof Reflect.get(value.placement, 'create') !== 'function' ||
    typeof value.previous !== 'function' ||
    typeof value.reportError !== 'function' ||
    (typeof value.owner !== 'object' && typeof value.owner !== 'function') ||
    value.owner === null
  ) {
    return null;
  }
  return value as unknown as SugarCreationWrapperState;
}

/** Apply the same title/options contract Comfy expects from LiteGraph.createNode. */
function applyHostCreationOptions(
  node: CubeNode,
  title: string | undefined,
  options: UnknownRecord | undefined,
): void {
  if (typeof title === 'string' && title.trim()) node.title = title;
  if (options) Object.assign(node, options);
}

/** Read a finite graph-space position from current Comfy creation options. */
function readPosition(value: unknown): Vec2 | null {
  if (!Array.isArray(value) || value.length < 2) return null;
  const x = Number(value[0]);
  const y = Number(value[1]);
  return Number.isFinite(x) && Number.isFinite(y) ? [x, y] : null;
}

/** Normalize one caught placement error for Sugar feedback. */
function readErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) return error.message.trim();
  return String(error || 'Unknown SugarCube placement failure');
}
