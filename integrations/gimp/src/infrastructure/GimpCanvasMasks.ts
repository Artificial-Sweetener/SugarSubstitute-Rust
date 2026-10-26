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
/** Translate the shared mask capability into persisted GIMP channels and native modeless editing. */
import type { CanvasMaskHost } from '../../../contracts/CanvasMasks.js';
import type {
  CanvasMaskQuery,
  CanvasMaskReference,
  CanvasMaskResult,
  CanvasMaskSnapshot,
} from '../../../contracts/CanvasMasksProtocol.generated.js';
import { CanvasCancellation } from '../../../shared/CanvasCancellation.js';
import { decodeMaskPixels, MAX_MASK_PIXELS } from '../../../shared/CanvasMaskPixels.js';
import { sameCanvasSourceDocument } from '../../../shared/CanvasSourceIdentity.js';
import type { GimpNativePort } from '../application/NativeGimpSession.js';
import { isNativeTattoo } from './NativeValidation.generated.js';
import {
  GimpNativeFailure,
  gimpCanvasDocument,
  readGimpContext,
  rejectNativeResult,
} from './GimpCanvasMetadata.js';
import { GimpNativeCapture } from './GimpNativeCapture.js';

/** Retain native pixels in the XCF while the shared application owns mask binding and execution. */
export class GimpCanvasMasks implements CanvasMaskHost {
  readonly canvasId = 'gimp';
  readonly changes = null;
  readonly #capture: GimpNativeCapture;
  #tail: Promise<unknown> = Promise.resolve();
  #active: CanvasCancellation | null = null;
  #closed = false;

  /** Borrow viewer-independent native IO and preserve actionable adapter diagnostics. */
  constructor(
    private readonly native: GimpNativePort,
    private readonly report: (error: unknown) => void,
  ) {
    this.#capture = new GimpNativeCapture(native);
  }

  /** Serialize shared commands without polling pixels or guessing native committed-change events. */
  execute(command: CanvasMaskQuery): Promise<CanvasMaskResult> {
    const pending = this.#tail.then(async (): Promise<CanvasMaskResult> => {
      if (this.#closed)
        return { kind: 'failure', status: 'unavailable', message: 'GIMP mask editing closed.' };
      const cancellation = new CanvasCancellation();
      this.#active = cancellation;
      try {
        return await this.#execute(command, cancellation);
      } catch (error: unknown) {
        if (error instanceof GimpNativeFailure)
          return { kind: 'failure', status: error.status, message: error.message };
        this.report(error);
        return { kind: 'failure', status: 'error', message: 'GIMP mask operation failed.' };
      } finally {
        this.#active = null;
      }
    });
    this.#tail = pending;
    return pending;
  }

  /** Retire requests and offscreen batches while the native process retains authored channel data. */
  dispose(): void {
    if (this.#closed) return;
    this.#closed = true;
    this.#active?.cancel();
    void this.#tail.then(() => this.#capture.close()).catch(this.report);
  }

  /** Apply existing shared mask semantics to only the companion's explicitly bound image. */
  async #execute(
    command: CanvasMaskQuery,
    cancellation: CanvasCancellation,
  ): Promise<CanvasMaskResult> {
    if (command.type === 'release' && command.mask === null) {
      const released = await this.native.execute(
        { type: 'channel.release', tattoo: null },
        cancellation,
      );
      if (released.kind !== 'channel-selected') rejectNativeResult(released);
      return { kind: 'activated' };
    }
    const current = await readGimpContext(this.native, cancellation);
    const document = current.document ? gimpCanvasDocument(current.hostId, current.document) : null;
    if (command.type === 'context')
      return { kind: 'context', document, supported: true, observesChanges: false };
    if (!current.document || !document)
      throw new GimpNativeFailure('stale', 'The GIMP mask document has closed.');
    const expected =
      command.type === 'create'
        ? command.document
        : command.type === 'capture'
          ? command.masks[0]?.document
          : command.mask?.document;
    if (expected && !sameCanvasSourceDocument(document, expected))
      throw new GimpNativeFailure('stale', 'The bound GIMP mask document geometry changed.');
    if (command.type === 'create') {
      if (command.source === 'pixels') {
        if (
          !command.pixels ||
          command.pixels.width !== document.width ||
          command.pixels.height !== document.height
        )
          throw new Error('The mask and GIMP document dimensions differ.');
        decodeMaskPixels(command.pixels);
      }
      const created = await this.native.execute(
        {
          type: 'channel.create',
          document: current.document,
          name: `SugarCubes ${command.name}`,
          source: command.source,
          data: command.source === 'pixels' ? (command.pixels?.data ?? null) : null,
        },
        cancellation,
      );
      if (created.kind !== 'channel-created') rejectNativeResult(created);
      return { kind: 'created', mask: { canvasId: 'gimp', document, id: String(created.tattoo) } };
    }
    if (command.type === 'activate' || command.type === 'release') {
      if (!command.mask) return { kind: 'activated' };
      const tattoo = this.#tattoo(command.mask, document);
      const result = await this.native.execute(
        command.type === 'activate'
          ? { type: 'channel.select', document: current.document, tattoo }
          : { type: 'channel.release', tattoo },
        cancellation,
      );
      if (result.kind !== 'channel-selected') rejectNativeResult(result);
      return { kind: 'activated' };
    }
    const tattoos = command.masks.map((mask) => this.#tattoo(mask, document));
    const scale = command.maximumSize
      ? Math.min(1, command.maximumSize / Math.max(document.width, document.height))
      : 1;
    const width = Math.max(1, Math.round(document.width * scale));
    const height = Math.max(1, Math.round(document.height * scale));
    if (width * height * tattoos.length > MAX_MASK_PIXELS)
      throw new Error('The mask collection exceeds its supported capture size.');
    return this.#capture.run(current.document, cancellation, async (snapshot) => {
      const snapshots: CanvasMaskSnapshot[] = [];
      for (const [index, mask] of command.masks.entries()) {
        const tattoo = tattoos[index];
        if (tattoo === undefined) throw new Error('Missing admitted GIMP mask identity.');
        const pixels = await this.native.execute(
          { type: 'snapshot.channel', token: snapshot.token, tattoo, width, height, scale },
          cancellation,
        );
        if (pixels.kind !== 'pixels') rejectNativeResult(pixels);
        if (pixels.format !== 'coverage8' || pixels.width !== width || pixels.height !== height)
          throw new Error('The native GIMP mask differs from its admitted pixel geometry.');
        const coverage = { width, height, data: pixels.data };
        decodeMaskPixels(coverage);
        snapshots.push({ mask, revision: snapshot.token, pixels: coverage });
      }
      return { kind: 'captured', snapshots };
    });
  }

  /** Refuse another canvas, document or noncanonical channel identity before native admission. */
  #tattoo(mask: CanvasMaskReference, document: CanvasMaskReference['document']): number {
    const tattoo = Number(mask.id);
    if (
      mask.canvasId !== 'gimp' ||
      !sameCanvasSourceDocument(document, mask.document) ||
      !isNativeTattoo(tattoo) ||
      String(tattoo) !== mask.id
    )
      throw new GimpNativeFailure('stale', 'The bound GIMP mask is unavailable.');
    return tattoo;
  }
}
