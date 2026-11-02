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
/** Own shared estimated-byte and provisional unknown-size admission accounting. */

import type { ImageDimensions } from '../../execution/domain/WorkflowOutput.js';
import { canvasImageBytes } from '../domain/CanvasImageResource.js';
import type { MemoryConsumer } from '../../core/memory/MemoryBudgetRegistry.js';

/** Bound default owned RGBA estimates, including outgoing and incoming visible images. */
export const CANVAS_IMAGE_BYTE_LIMIT = 128 * 1024 * 1024;

export interface CanvasImageReservation {
  readonly id: number;
}

interface Admission {
  bytes: number | null;
  readonly thumbnail: boolean;
  readonly approvedUnknown: boolean;
  readonly epoch: number;
}

/** Account every displayed and incoming image through one reservation authority. */
export class CanvasImageBudget {
  readonly #reservations = new Map<CanvasImageReservation, Admission>();
  #next = 0;
  #allowance = CANVAS_IMAGE_BYTE_LIMIT;
  #unknownAllowance = 0;
  #epoch = 0;

  /** Set an explicitly approved current-selection allowance without granting a global opt-out. */
  allowance(bytes: number, unknownLoads = 0): void {
    if (
      !Number.isSafeInteger(bytes) ||
      bytes < 0 ||
      !Number.isSafeInteger(unknownLoads) ||
      unknownLoads < 0 ||
      unknownLoads > 2
    )
      throw new RangeError('Canvas image allowance is invalid.');
    this.#allowance = Math.max(CANVAS_IMAGE_BYTE_LIMIT, bytes);
    this.#unknownAllowance = unknownLoads;
    this.#epoch++;
  }

  /** Reserve known estimates or the single provisional unknown-size load before decoding. */
  reserve(dimensions: ImageDimensions | null, thumbnail: boolean): CanvasImageReservation | null {
    const bytes = dimensions ? canvasImageBytes(dimensions) : null;
    if (thumbnail && this.#allowance > CANVAS_IMAGE_BYTE_LIMIT) return null;
    if (
      bytes === null &&
      (this.unknownLoads > 0 ||
        (this.estimatedBytes >= this.#allowance && this.#unknownAllowance === 0))
    )
      return null;
    if (bytes !== null && this.estimatedBytes + bytes > this.#allowance) return null;
    const reservation = Object.freeze({ id: ++this.#next });
    const approvedUnknown = bytes === null && !thumbnail && this.#unknownAllowance > 0;
    if (approvedUnknown) this.#unknownAllowance--;
    this.#reservations.set(reservation, { bytes, thumbnail, approvedUnknown, epoch: this.#epoch });
    return reservation;
  }

  /** Reconcile actual decoded geometry before a frame becomes displayable. */
  measure(reservation: CanvasImageReservation, dimensions: ImageDimensions): boolean {
    const admission = this.#reservations.get(reservation);
    if (!admission) return false;
    const bytes = canvasImageBytes(dimensions);
    const unknown = admission.bytes === null;
    admission.bytes = bytes;
    if (unknown && admission.approvedUnknown && admission.epoch === this.#epoch)
      this.#allowance = Math.max(this.#allowance, this.estimatedBytes);
    return (
      this.estimatedBytes <= this.#allowance &&
      (!admission.thumbnail || this.#allowance === CANVAS_IMAGE_BYTE_LIMIT)
    );
  }

  /** Reclassify an already admitted frame without allocating or bypassing the current allowance. */
  reclassify(reservation: CanvasImageReservation, thumbnail: boolean): boolean {
    const admission = this.#reservations.get(reservation);
    if (
      !admission ||
      this.estimatedBytes > this.#allowance ||
      (thumbnail && !this.thumbnailsAllowed)
    )
      return false;
    this.#reservations.set(reservation, { ...admission, thumbnail });
    return true;
  }

  /** Release obsolete accounting exactly once, including late cancellation cleanup. */
  release(reservation: CanvasImageReservation): void {
    const admission = this.#reservations.get(reservation);
    if (admission?.bytes === null && admission.approvedUnknown && admission.epoch === this.#epoch)
      this.#unknownAllowance++;
    this.#reservations.delete(reservation);
  }

  /** Read only the owned decoded estimates, excluding browser-controlled overhead. */
  get estimatedBytes(): number {
    let bytes = 0;
    for (const admission of this.#reservations.values()) bytes += admission.bytes ?? 0;
    return bytes;
  }

  /** Count provisional loads whose intrinsic dimensions have not yet been reconciled. */
  get unknownLoads(): number {
    let count = 0;
    for (const admission of this.#reservations.values()) if (admission.bytes === null) count++;
    return count;
  }

  /** Identify staging that must avoid overlapping an oversized outgoing image. */
  get oversized(): boolean {
    return this.estimatedBytes > CANVAS_IMAGE_BYTE_LIMIT;
  }

  /** Suspend overview allocations while explicitly approved oversized main images are inspected. */
  get thumbnailsAllowed(): boolean {
    return this.#allowance === CANVAS_IMAGE_BYTE_LIMIT;
  }

  /**
   * Report displayed and incoming images to the page allowance.
   *
   * These images are on screen or about to be, so they are pinned: they count against the shared
   * allowance and push other caches out, but are never evicted by it.
   */
  memoryConsumer(id: string): MemoryConsumer {
    return {
      id,
      bytes: () => this.estimatedBytes,
      oldestEvictable: () => null,
      evict: () => 0,
    };
  }

  /** Release the integration's reservations and explicit selection allowance. */
  dispose(): void {
    this.#reservations.clear();
    this.#allowance = CANVAS_IMAGE_BYTE_LIMIT;
    this.#unknownAllowance = 0;
    this.#epoch++;
  }
}
