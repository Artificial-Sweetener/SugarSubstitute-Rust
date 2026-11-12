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
/** Remember one explicitly approved image/pair estimate per open browser view. */

import type { CanvasViewId } from '../domain/CanvasIdentity.js';
import type { ImageDimensions } from '../../execution/domain/WorkflowOutput.js';
import { canvasImageBytes } from '../domain/CanvasImageResource.js';
import { CANVAS_IMAGE_BYTE_LIMIT } from './CanvasImageBudget.js';

interface ImageAllowance {
  readonly approvalId: number;
  readonly generation: number;
  readonly key: string;
  readonly images: readonly string[];
  readonly dimensions: ReadonlyMap<string, ImageDimensions | null>;
  readonly bytes: number;
  readonly unknownLoads: number;
}

export interface CanvasImageAllowanceStatus {
  readonly approvalId: number;
  readonly bytes: number;
  readonly unknownLoads: number;
}

export interface CanvasImageAllowanceInput {
  readonly key: string;
  readonly dimensions: ImageDimensions | null;
}

/** Keep approval scoped to the exact current result identities, with no global large-image setting. */
export class CanvasImageAllowances {
  readonly #views = new Map<CanvasViewId, ImageAllowance>();
  #nextApproval = 0;

  /** Approve only the selected image/pair, replacing any older approval in that view. */
  grant(
    view: CanvasViewId,
    generation: number,
    images: readonly CanvasImageAllowanceInput[],
  ): void {
    if (images.length < 1 || images.length > 2)
      throw new RangeError('Canvas large-image approval requires one image or a pair.');
    const dimensions = new Map(
      images.map(({ key, dimensions: size }) => [key, size ? Object.freeze({ ...size }) : null]),
    );
    const bytes = images.reduce(
      (sum, image) => sum + (image.dimensions ? canvasImageBytes(image.dimensions) : 0),
      0,
    );
    if (!Number.isSafeInteger(bytes))
      throw new RangeError('Canvas image pair estimate is invalid.');
    this.#views.set(view, {
      approvalId: ++this.#nextApproval,
      generation,
      key: allowanceKey(images.map((image) => image.key)),
      images: Object.freeze(images.map((image) => image.key)),
      dimensions,
      bytes,
      unknownLoads: images.filter((image) => image.dimensions === null).length,
    });
  }

  /** Return approval only for the exact generation and image set, preserving it through A/B swaps. */
  read(
    view: CanvasViewId,
    generation: number,
    images: readonly string[],
  ): CanvasImageAllowanceStatus | null {
    const allowance = this.#views.get(view);
    return allowance?.generation === generation && allowance.key === allowanceKey(images)
      ? Object.freeze({
          approvalId: allowance.approvalId,
          bytes: Math.max(CANVAS_IMAGE_BYTE_LIMIT, allowance.bytes),
          unknownLoads: allowance.unknownLoads,
        })
      : null;
  }

  /** Reconcile only approved unknown geometry after successful retention, without widening known approval. */
  measure(
    view: CanvasViewId,
    generation: number,
    image: string,
    dimensions: ImageDimensions,
  ): void {
    const allowance = this.#views.get(view);
    if (allowance?.generation !== generation || allowance.dimensions.get(image) !== null) return;
    const images = new Map(allowance.dimensions);
    images.set(image, Object.freeze({ ...dimensions }));
    const bytes = allowance.images.reduce((sum, key) => {
      const size = images.get(key);
      return sum + (size ? canvasImageBytes(size) : 0);
    }, 0);
    if (!Number.isSafeInteger(bytes))
      throw new RangeError('Canvas image pair estimate is invalid.');
    this.#views.set(view, {
      ...allowance,
      dimensions: images,
      bytes,
      unknownLoads: allowance.images.filter((key) => images.get(key) === null).length,
    });
  }

  /** Recover only bounded lightweight geometry for an approved image after tab reactivation. */
  dimensions(view: CanvasViewId, generation: number, image: string): ImageDimensions | null {
    const allowance = this.#views.get(view);
    return allowance?.generation === generation ? (allowance.dimensions.get(image) ?? null) : null;
  }

  /** Release an explicitly closed or replaced view's approval. */
  remove(view: CanvasViewId): void {
    this.#views.delete(view);
  }

  /** End this integration without retaining approval identities. */
  dispose(): void {
    this.#views.clear();
  }
}

/** Qualify a set of selected results without making wipe orientation affect admission. */
function allowanceKey(images: readonly string[]): string {
  return JSON.stringify([...images].sort());
}
