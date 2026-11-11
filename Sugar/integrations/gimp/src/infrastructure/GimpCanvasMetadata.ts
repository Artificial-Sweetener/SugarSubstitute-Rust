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
/** Translate SDK document metadata into the existing portable canvas identities. */
import type {
  CanvasDocument,
  CanvasSourceLayer,
} from '../../../contracts/CanvasProtocol.generated.js';
import type { GimpNativePort } from '../application/NativeGimpSession.js';
import type {
  NativeDocument,
  NativeLayer,
  NativeResult,
} from '../contracts/NativeProtocol.generated.js';
import type { CanvasCancellationSource } from '../../../contracts/CanvasHost.js';

export type GimpContext = Extract<NativeResult, { kind: 'context' }>;

/** Carry expected native refusals without turning them into competing workflow decisions. */
export class GimpNativeFailure extends Error {
  /** Retain the bounded status emitted by the private native adapter. */
  constructor(
    readonly status: Extract<NativeResult, { kind: 'failure' }>['status'],
    message: string,
  ) {
    super(message);
  }
}

/** Refuse an unexpected primitive reply before interpreting it as metadata or committed output. */
export function rejectNativeResult(result: NativeResult): never {
  if (result.kind === 'failure') throw new GimpNativeFailure(result.status, result.message);
  throw new Error('The GIMP native adapter returned an unexpected operation result.');
}

/** Read the bound launch image rather than inferring active-document identity from a native inventory. */
export async function readGimpContext(
  native: GimpNativePort,
  cancellation?: CanvasCancellationSource,
): Promise<GimpContext> {
  const context = await native.execute({ type: 'context' }, cancellation);
  if (context.kind !== 'context') rejectNativeResult(context);
  return context;
}

/** Distinguish native process restarts while preserving identity across companion renderer reloads. */
export function gimpCanvasDocument(hostId: string, document: NativeDocument): CanvasDocument {
  return {
    id: `${hostId}:${String(document.imageId)}`,
    geometryRevision: document.revision,
    width: document.width,
    height: document.height,
  };
}

/** Preserve native preorder and parent relationships in the shared layer catalog. */
export function gimpCanvasLayers(layers: readonly NativeLayer[]): readonly CanvasSourceLayer[] {
  return layers.map((layer) => ({
    id: String(layer.tattoo),
    parentId: layer.parentTattoo === null ? null : String(layer.parentTattoo),
    name: layer.name,
    depth: layer.depth,
    visible: layer.visible,
  }));
}
