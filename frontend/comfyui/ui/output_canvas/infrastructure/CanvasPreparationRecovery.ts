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
/** Retry a timed-out surface preparation for an unchanged target on a bounded backoff. */
import type { ExecutionClock } from '../../execution/domain/WorkflowExecutionEvent.js';
import type { CanvasViewId } from '../domain/CanvasIdentity.js';

/** Delays before each fresh attempt; after the last, the target shows its error state. */
export const CANVAS_PREPARATION_RETRY_DELAYS: readonly number[] = Object.freeze([1000, 2000, 4000]);

/** Identify the attachment target a retry must still match. */
export interface CanvasPreparationTarget {
  readonly view: CanvasViewId;
  readonly generation: number;
  readonly element: HTMLElement;
  readonly renderer: string;
}

export interface CanvasPreparationRecoveryOptions {
  readonly clock: Pick<ExecutionClock, 'later'>;
  /** Attach the same target afresh. */
  retry(target: CanvasPreparationTarget): void;
  /** Stop retrying; the target's attachment shows its preparation error. */
  exhausted(target: CanvasPreparationTarget): void;
}

/**
 * Own the attempt count and the single backoff deadline for one attachment target.
 *
 * Each preparation timeout of the same target schedules the next fresh attempt after a longer
 * delay through the injected clock; a different target, a successful attachment or an explicit
 * reset starts over. Only one deadline is ever pending.
 */
export class CanvasPreparationRecovery {
  #target: CanvasPreparationTarget | null = null;
  #attempts = 0;
  #stop: (() => void) | null = null;

  /** Bind the clock and the attachment owner's retry and exhaustion actions. */
  constructor(private readonly options: CanvasPreparationRecoveryOptions) {}

  /** Schedule the next fresh attempt after a timeout, or report exhaustion. */
  failed(target: CanvasPreparationTarget): void {
    if (!this.#target || !sameCanvasPreparationTarget(this.#target, target)) this.reset();
    this.#target = target;
    this.#stop?.();
    this.#stop = null;
    const delay = CANVAS_PREPARATION_RETRY_DELAYS[this.#attempts];
    this.#attempts += 1;
    if (delay === undefined) {
      this.options.exhausted(target);
      return;
    }
    this.#stop = this.options.clock.later(delay, () => {
      this.#stop = null;
      this.options.retry(target);
    });
  }

  /** Forget attempts and any pending retry after success or a changed request. */
  reset(): void {
    this.#stop?.();
    this.#stop = null;
    this.#target = null;
    this.#attempts = 0;
  }
}

/** Compare every target member, including the exact mounted element. */
export function sameCanvasPreparationTarget(
  left: CanvasPreparationTarget,
  right: CanvasPreparationTarget,
): boolean {
  return (
    left.view === right.view &&
    left.generation === right.generation &&
    left.element === right.element &&
    left.renderer === right.renderer
  );
}
