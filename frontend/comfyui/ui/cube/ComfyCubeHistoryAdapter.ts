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
/** Adapt Cube mutations to Comfy's authoritative workflow-history boundary. */

import type { UnknownRecord } from '../types/common.js';

export interface ComfyCubeHistory {
  beforeChange(): void;
  afterChange(): void;
  setDirtyCanvas(foreground?: boolean, background?: boolean): void;
}

export interface ComfyCubeHistoryAdapterOptions {
  canvas: UnknownRecord;
  graph: UnknownRecord;
}

/** Resolve one version-aware history adapter at the Comfy host boundary. */
export function createComfyCubeHistoryAdapter(
  options: ComfyCubeHistoryAdapterOptions,
): ComfyCubeHistory {
  const canvasBefore = readMethod(options.canvas, 'emitBeforeChange');
  const canvasAfter = readMethod(options.canvas, 'emitAfterChange');
  const graphBefore = readMethod(options.graph, 'beforeChange');
  const graphAfter = readMethod(options.graph, 'afterChange');
  const canvasDirty = readMethod(options.canvas, 'setDirty');
  const graphDirty = readMethod(options.graph, 'setDirtyCanvas');
  const useCanvasTransactions = canvasBefore !== null && canvasAfter !== null;

  if (!useCanvasTransactions && (!graphBefore || !graphAfter)) {
    throw new TypeError('Comfy workflow history transaction methods are unavailable.');
  }
  if (!canvasDirty && !graphDirty) {
    throw new TypeError('Comfy canvas invalidation method is unavailable.');
  }

  return {
    beforeChange() {
      if (useCanvasTransactions) {
        canvasBefore();
      } else {
        graphBefore?.();
      }
    },
    afterChange() {
      if (useCanvasTransactions) {
        canvasAfter();
      } else {
        graphAfter?.();
      }
    },
    setDirtyCanvas(foreground, background) {
      if (canvasDirty) {
        canvasDirty(foreground, background);
      } else {
        graphDirty?.(foreground, background);
      }
    },
  };
}

/** Read one optional dynamic host method without leaking an untyped callable. */
function readMethod(owner: UnknownRecord, name: string): ((...args: unknown[]) => unknown) | null {
  const value = owner[name];
  if (typeof value !== 'function') return null;
  return (...args: unknown[]) => Reflect.apply(value, owner, args);
}
