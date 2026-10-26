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
/** Translate portable geometry commands into guarded native canvas-bounds operations. */
import { app, core, constants } from 'photoshop';
import type { CanvasGeometryHost } from '../../../contracts/CanvasGeometry.js';
import type { CanvasCancellationSource } from '../../../contracts/CanvasHost.js';
import type {
  CanvasDocument,
  CanvasGeometryCommand,
  CanvasGeometryResult,
} from '../../../contracts/CanvasProtocol.generated.js';
import { canvasDocumentIdentity, findCanvasDocument } from './PhotoshopCanvasIdentity.js';
type ModalExecution = Parameters<Parameters<typeof core.executeAsModal>[0]>[0];

/** Own native document geometry reads and coherent undo without generation or scaling policy. */
export class PhotoshopCanvasGeometry implements CanvasGeometryHost {
  readonly canvasId = 'photoshop';
  /** Revalidate externally mutable identities before entering UI or modifying native canvas bounds. */
  async execute(
    command: CanvasGeometryCommand,
    cancellation?: CanvasCancellationSource,
  ): Promise<CanvasGeometryResult> {
    if (cancellation?.cancelled())
      return { kind: 'failure', status: 'cancelled', message: 'Canvas sizing cancelled.' };
    if (command.type === 'context') {
      const target = app.activeDocument;
      const bounds = target?.selection.bounds;
      const left = bounds ? Math.floor(bounds.left) : 0;
      const top = bounds ? Math.floor(bounds.top) : 0;
      const width = bounds ? Math.ceil(bounds.right) - left : 0;
      const height = bounds ? Math.ceil(bounds.bottom) - top : 0;
      return {
        kind: 'context',
        supported: true,
        resize: !target?.artboards.length && !target?.quickMaskMode,
        selectionSupported: true,
        document: target ? canvasDocumentIdentity(target) : null,
        selection: width > 0 && height > 0 ? { left, top, width, height } : null,
      };
    }
    const target = this.#target(command.document);
    if (!target)
      return {
        kind: 'failure',
        status: 'stale',
        message: 'The destination document closed or its canvas geometry changed.',
      };
    if (
      ![command.size.width, command.size.height].every(
        (value) => Number.isSafeInteger(value) && value > 0 && value <= 300000,
      )
    )
      return {
        kind: 'failure',
        status: 'error',
        message: 'Canvas dimensions must be positive pixel integers within the supported range.',
      };
    if (target.artboards.length || target.quickMaskMode)
      return {
        kind: 'failure',
        status: 'unsupported',
        message: 'Canvas resizing requires a document without artboards or Quick Mask.',
      };
    let result: CanvasGeometryResult = {
      kind: 'failure',
      status: 'cancelled',
      message: 'Canvas sizing cancelled.',
    };
    await core.executeAsModal(
      async (execution) => {
        if (this.#cancelled(execution, cancellation)) return;
        if (!this.#target(command.document)) {
          result = {
            kind: 'failure',
            status: 'stale',
            message: 'The canvas geometry changed before resizing.',
          };
          return;
        }
        const history = await execution.hostControl.suspendHistory({
          documentID: target.id,
          name: 'Resize canvas',
        });
        try {
          await target.resizeCanvas(
            command.size.width,
            command.size.height,
            constants.AnchorPosition.MIDDLECENTER,
          );
          if (this.#cancelled(execution, cancellation)) {
            await execution.hostControl.resumeHistory(history, false);
            return;
          }
          if (target.width !== command.size.width || target.height !== command.size.height)
            throw new Error('Photoshop did not apply the requested canvas dimensions.');
          await execution.hostControl.resumeHistory(history, true);
          result = { kind: 'resized', document: canvasDocumentIdentity(target) };
        } catch (error: unknown) {
          if (!this.#cancelled(execution)) {
            try {
              await execution.hostControl.resumeHistory(history, false);
            } catch (rollback: unknown) {
              throw new AggregateError([error, rollback], 'Canvas sizing and rollback failed.');
            }
          }
          throw error;
        }
      },
      { commandName: 'Resize canvas', timeOut: 2000 },
    );
    return result;
  }

  /** Resolve a captured identity without substituting whichever document happens to be active. */
  #target(expected: CanvasDocument) {
    const target = findCanvasDocument(expected.id);
    if (!target) return null;
    const current = canvasDocumentIdentity(target);
    return current.width === expected.width &&
      current.height === expected.height &&
      current.geometryRevision === expected.geometryRevision
      ? target
      : null;
  }

  /** Re-read native cancellation after each awaited operation rather than retaining a narrowed snapshot. */
  #cancelled(execution: ModalExecution, cancellation?: CanvasCancellationSource): boolean {
    return execution.isCancelled || Boolean(cancellation?.cancelled());
  }
}
