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
/** Own one visible lane's queued payload, pending lease and releasable displayed frame. */

import type { CanvasResourceStatus } from '../domain/CanvasViewPort.js';
import type { CanvasSelection } from '../domain/CanvasViewState.js';
import type {
  CanvasDecodedImage,
  CanvasImageFrame,
  CanvasImageInput,
} from '../domain/CanvasImageResource.js';
import { CanvasImageBudget, type CanvasImageReservation } from './CanvasImageBudget.js';
import type { CanvasDecodeLease } from './CanvasImageDecodeScheduler.js';

export interface CanvasImageIntent {
  readonly key: string;
  readonly selection: CanvasSelection;
  readonly lane: string;
  readonly fallbackKey: string | null;
  readonly preview: boolean;
  readonly role: 'main' | 'thumbnail';
}
export interface CanvasDisplayedImage<Handle> {
  readonly frame: CanvasImageFrame<Handle>;
  readonly lane: string;
  readonly image: CanvasDecodedImage<Handle>;
  readonly reservation: CanvasImageReservation;
}
interface Pending {
  readonly intent: CanvasImageIntent;
  readonly reservation: CanvasImageReservation;
  readonly lease: CanvasDecodeLease;
}

/** Keep exact cancellation and release transitions separate from multi-slot scheduling. */
export class CanvasImageSlot<Payload, Handle> {
  wanted: CanvasImageIntent;
  queued: CanvasImageInput<Payload> | null = null;
  pending: Pending | null = null;
  displayed: CanvasDisplayedImage<Handle> | null = null;
  status = canvasResourceStatus('empty', '');
  lastPreview = -Infinity;

  /** Bind the shared accounting authority without taking ownership of another lane. */
  constructor(
    input: CanvasImageInput<Payload>,
    private readonly budget: CanvasImageBudget,
  ) {
    this.wanted = canvasImageIntent(input);
    this.status = canvasResourceStatus('loading', 'Loading image…');
  }

  /** Replace interest, allowing a current preview decode to complete despite faster incoming frames. */
  update(input: CanvasImageInput<Payload>, retry: boolean): number {
    if (
      this.wanted.key === input.key &&
      (!retry || !['budget', 'error', 'empty'].includes(this.status.state))
    )
      return 0;
    if (this.wanted.lane !== input.lane) this.lastPreview = -Infinity;
    let drops = this.queued?.preview && this.queued.key !== input.key ? 1 : 0;
    if (
      this.pending &&
      (!input.preview || !this.pending.intent.preview || this.pending.intent.lane !== input.lane)
    )
      drops += this.cancel();
    if (this.displayed?.frame.preview && (!input.preview || this.displayed.lane !== input.lane))
      this.dropDisplay();
    this.wanted = canvasImageIntent(input);
    if (!input.preview && this.displayed?.frame.key === input.key) {
      this.queued = null;
      this.status = canvasResourceStatus('ready', '');
    } else {
      this.queued = input;
      this.status = canvasResourceStatus(
        'loading',
        input.preview ? 'Loading source preview…' : 'Loading image…',
      );
    }
    return drops;
  }

  /** Transfer a final frame without releasing its handle or decoding a duplicate. */
  takeDisplay(): CanvasDisplayedImage<Handle> | null {
    const display = this.displayed;
    this.displayed = null;
    return display;
  }

  /** Adopt a transferred frame after dropping this lane's obsolete displayed ownership. */
  adopt(displayed: CanvasDisplayedImage<Handle>): number {
    const drops = this.cancel();
    this.dropDisplay();
    this.displayed = displayed;
    this.status = canvasResourceStatus('ready', '');
    return drops;
  }

  /** Cancel the exact pending admission before replacing its source or run. */
  cancel(): number {
    const pending = this.pending;
    if (!pending) return 0;
    this.pending = null;
    pending.lease.cancel();
    this.budget.release(pending.reservation);
    return pending.intent.preview ? 1 : 0;
  }

  /** Release a decoded handle and its reservation exactly once. */
  dropDisplay(): void {
    const displayed = this.takeDisplay();
    if (!displayed) return;
    displayed.image.release();
    this.budget.release(displayed.reservation);
  }

  /** Release queued bytes, pending transport and displayed pixels together. */
  release(): number {
    let drops = this.queued?.preview ? 1 : 0;
    this.queued = null;
    drops += this.cancel();
    this.dropDisplay();
    return drops;
  }
}

/** Copy light routing metadata without retaining opaque payloads in pending/displayed records. */
export function canvasImageIntent<Payload>(input: CanvasImageInput<Payload>): CanvasImageIntent {
  return Object.freeze({
    key: input.key,
    selection: Object.freeze({ ...input.selection }),
    lane: input.lane,
    fallbackKey: input.fallbackKey,
    preview: input.preview,
    role: input.role,
  });
}

/** Publish explicit recoverable state through the shared overlay contract. */
export function canvasResourceStatus(
  state: CanvasResourceStatus['state'],
  message: string,
  canRetry = false,
  canAdmitLargeImage = false,
): CanvasResourceStatus {
  return Object.freeze({ state, message, canRetry, canAdmitLargeImage });
}
