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
/** Retain a thumbnail reveal requested while a native attachment is still preparing. */
import type { CanvasRevealTarget } from '../domain/CanvasReveal.js';

export interface NativeCanvasPendingRevealOptions {
  /** Apply a reveal through the attachment's committed command port. */
  dispatch(target: CanvasRevealTarget): Promise<boolean>;
  /** Read the attachment view's current state revision. */
  revision(): number;
  report(error: unknown): void;
}

interface PendingReveal {
  readonly target: CanvasRevealTarget;
  /** The view state revision the reveal was requested against. */
  readonly revision: number;
  readonly settle: (applied: boolean) => void;
}

/**
 * Hold only the latest reveal until the staged attachment accepts commands.
 *
 * A browser attachment awaits source preparation before applying a reveal; a native candidate
 * cannot accept commands until it commits, so it defers the same intent instead of refusing it.
 * A newer reveal, any newer view state (another selection, mode or comparison) and disposal
 * each settle the retained reveal as not applied.
 */
export class NativeCanvasPendingReveal {
  #pending: PendingReveal | null = null;

  /** Bind the attachment's command port and view state authority. */
  constructor(private readonly options: NativeCanvasPendingRevealOptions) {}

  /** Replace any older retained reveal and resolve once this one is applied or dropped. */
  retain(target: CanvasRevealTarget): Promise<boolean> {
    this.drop();
    const revision = this.options.revision();
    return new Promise<boolean>((settle) => {
      this.#pending = { target, revision, settle };
    });
  }

  /** Apply the retained reveal once the attachment accepts, unless the view changed since. */
  apply(): void {
    const pending = this.#pending;
    this.#pending = null;
    if (!pending) return;
    if (pending.revision !== this.options.revision()) {
      pending.settle(false);
      return;
    }
    void this.options.dispatch(pending.target).then(pending.settle, (error: unknown) => {
      this.options.report(error);
      pending.settle(false);
    });
  }

  /** Settle a retained reveal as not applied. */
  drop(): void {
    const pending = this.#pending;
    this.#pending = null;
    pending?.settle(false);
  }
}
