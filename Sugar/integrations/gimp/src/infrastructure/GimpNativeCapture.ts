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
/** Own coherent native batches and retry failed releases for every GIMP capture consumer. */
import type { CanvasCancellationSource } from '../../../contracts/CanvasHost.js';
import type { GimpNativePort } from '../application/NativeGimpSession.js';
import type { NativeDocument, NativeResult } from '../contracts/NativeProtocol.generated.js';
import { GimpNativeFailure, rejectNativeResult } from './GimpCanvasMetadata.js';

type Snapshot = Extract<NativeResult, { kind: 'snapshot' }>;

/** Retain offscreen native resources until the host acknowledges their actual release. */
export class GimpNativeCapture {
  readonly #retained = new Set<string>();

  /** Borrow native primitives without introducing a second capture or execution policy. */
  constructor(private readonly native: GimpNativePort) {}

  /** Freeze one exact native document and release it on success, failure, stale geometry or cancellation. */
  async run<T>(
    document: NativeDocument,
    cancellation: CanvasCancellationSource,
    operation: (snapshot: Snapshot) => Promise<T>,
  ): Promise<T> {
    await this.close();
    const snapshot = await this.native.execute({ type: 'snapshot.begin', document }, cancellation);
    if (snapshot.kind !== 'snapshot') rejectNativeResult(snapshot);
    this.#retained.add(snapshot.token);
    try {
      const actual = snapshot.document;
      if (
        actual.imageId !== document.imageId ||
        actual.revision !== document.revision ||
        actual.width !== document.width ||
        actual.height !== document.height
      )
        throw new GimpNativeFailure(
          'stale',
          'The GIMP canvas changed while its source was frozen.',
        );
      return await operation(snapshot);
    } finally {
      await this.#release(snapshot.token);
    }
  }

  /** Retry every failed release before admitting more native resources or retiring this consumer. */
  async close(): Promise<void> {
    const errors: unknown[] = [];
    for (const token of this.#retained) {
      try {
        await this.#release(token);
      } catch (error: unknown) {
        errors.push(error);
      }
    }
    if (errors.length) throw new AggregateError(errors, 'GIMP native capture cleanup failed.');
  }

  /** Forget ownership only after the native offscreen image has actually been released. */
  async #release(token: string): Promise<void> {
    const result = await this.native.execute({ type: 'snapshot.release', token });
    if (result.kind !== 'released') rejectNativeResult(result);
    this.#retained.delete(token);
  }
}
