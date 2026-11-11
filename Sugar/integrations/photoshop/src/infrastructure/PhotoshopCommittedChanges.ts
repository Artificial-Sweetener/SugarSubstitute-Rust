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
/** Observe committed history identity without reading coverage during pointer motion or drawing frames. */
declare function setInterval(callback: () => void, milliseconds: number): number;
declare function clearInterval(timer: number): void;
/** Read lightweight metadata while consumers subscribe and publish only committed state changes. */
export class PhotoshopCommittedChanges {
  #timer: number | null = null;
  #state: string | null = null;
  #pending = false;
  #disposed = false;
  /** Keep native metadata reads separate from consumer work and its exclusive modal lifetime. */
  constructor(
    private readonly readState: () => string | null,
    private readonly busy: () => boolean,
    private readonly changed: () => Promise<void>,
    private readonly report: (error: unknown) => void,
  ) {}
  /** Observe history at a bounded cadence; no pointer, brush or animation-frame subscription is involved. */
  start(): void {
    if (this.#timer !== null || this.#disposed) return;
    this.acceptCurrent();
    this.#timer = setInterval(this.#check, 500);
  }
  /** Exclude history written by this adapter from the next external-change comparison. */
  acceptCurrent(): void {
    this.#state = this.readState();
  }
  /** Stop idle observation while keeping the observer available for later subscribers. */
  stop(): void {
    if (this.#timer !== null) clearInterval(this.#timer);
    this.#timer = null;
    this.#state = null;
  }
  /** Release metadata observation when the native panel navigation ends. */
  dispose(): void {
    this.#disposed = true;
    this.stop();
  }
  /** Coalesce completed changes and leave coverage untouched while document history remains unchanged. */
  readonly #check = (): void => {
    if (this.#disposed || this.#timer === null || this.#pending || this.busy()) return;
    try {
      const state = this.readState();
      if (state === this.#state) return;
      this.#state = state;
      if (state === null) return;
      this.#pending = true;
      void this.changed()
        .catch((error: unknown) => {
          this.report(error);
        })
        .finally(() => {
          this.#pending = false;
        });
    } catch (error: unknown) {
      this.report(error);
    }
  };
}
