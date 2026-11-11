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
/** Translate shared sizing capabilities into native GIMP geometry primitives. */
import type { CanvasGeometryHost } from '../../../contracts/CanvasGeometry.js';
import type { CanvasCancellationSource } from '../../../contracts/CanvasHost.js';
import type {
  CanvasGeometryCommand,
  CanvasGeometryResult,
} from '../../../contracts/CanvasProtocol.generated.js';
import { sameCanvasSourceDocument } from '../../../shared/CanvasSourceIdentity.js';
import type { GimpNativePort } from '../application/NativeGimpSession.js';
import {
  GimpNativeFailure,
  gimpCanvasDocument,
  readGimpContext,
  rejectNativeResult,
} from './GimpCanvasMetadata.js';

/** Keep sizing arithmetic and menu policy in the shared application owners. */
export class GimpCanvasGeometry implements CanvasGeometryHost {
  readonly canvasId = 'gimp';

  /** Borrow the existing session without creating a separate product API. */
  constructor(private readonly native: GimpNativePort) {}

  /** Read or resize only the launch image qualified by its captured geometry. */
  async execute(
    command: CanvasGeometryCommand,
    cancellation?: CanvasCancellationSource,
  ): Promise<CanvasGeometryResult> {
    try {
      if (cancellation?.cancelled())
        return { kind: 'failure', status: 'cancelled', message: 'GIMP canvas sizing cancelled.' };
      const context = await readGimpContext(this.native, cancellation);
      const document = context.document
        ? gimpCanvasDocument(context.hostId, context.document)
        : null;
      if (!context.document || !document)
        return command.type === 'context'
          ? {
              kind: 'context',
              supported: true,
              resize: true,
              selectionSupported: true,
              document: null,
              selection: null,
            }
          : { kind: 'failure', status: 'stale', message: 'The captured GIMP document has closed.' };
      if (command.type === 'resize' && !sameCanvasSourceDocument(document, command.document))
        return { kind: 'failure', status: 'stale', message: 'The captured GIMP geometry changed.' };
      if (cancellation?.cancelled())
        return { kind: 'failure', status: 'cancelled', message: 'GIMP canvas sizing cancelled.' };
      if (command.type === 'context') {
        const result = await this.native.execute(
          { type: 'geometry.context', document: context.document },
          cancellation,
        );
        if (result.kind !== 'geometry') rejectNativeResult(result);
        return {
          kind: 'context',
          supported: true,
          resize: true,
          selectionSupported: true,
          document: gimpCanvasDocument(context.hostId, result.document),
          selection: result.selection,
        };
      }
      const result = await this.native.execute(
        {
          type: 'image.resize',
          document: context.document,
          width: command.size.width,
          height: command.size.height,
        },
        cancellation,
      );
      if (result.kind !== 'resized') rejectNativeResult(result);
      return { kind: 'resized', document: gimpCanvasDocument(context.hostId, result.document) };
    } catch (error: unknown) {
      if (error instanceof GimpNativeFailure)
        return { kind: 'failure', status: error.status, message: error.message };
      throw error;
    }
  }
}
