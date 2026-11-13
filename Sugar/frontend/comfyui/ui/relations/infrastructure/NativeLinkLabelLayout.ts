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
/** Batch native link label geometry before committing any presentation writes. */
export type NativeLinkLabelMeasure = () => (() => void) | null;

/** Own one cancellable frame for the renderer's pending label measurements. */
export class NativeLinkLabelLayout {
  readonly #pending = new Set<NativeLinkLabelMeasure>();
  #frame: number | null = null;

  /** Receive the rendering document and its actionable error boundary. */
  constructor(
    private readonly document: Document,
    private readonly reportError: (error: unknown) => void,
  ) {}

  /** Coalesce replacement and resize notifications without retaining inactive controls. */
  invalidate(measure: NativeLinkLabelMeasure): void {
    const windowRef = this.document.defaultView;
    if (!windowRef) return;
    this.#pending.add(measure);
    if (this.#frame !== null) return;
    this.#frame = windowRef.requestAnimationFrame(this.#flush);
  }

  /** Release pending work and its frame when the last control leaves. */
  release(measure: NativeLinkLabelMeasure): void {
    this.#pending.delete(measure);
    if (this.#pending.size || this.#frame === null) return;
    this.document.defaultView?.cancelAnimationFrame(this.#frame);
    this.#frame = null;
  }

  /** Complete every read before writes so siblings share one browser layout. */
  readonly #flush = (): void => {
    this.#frame = null;
    const measures = [...this.#pending];
    this.#pending.clear();
    const commits: Array<() => void> = [];
    for (const measure of measures) {
      try {
        const commit = measure();
        if (commit) commits.push(commit);
      } catch (error: unknown) {
        this.reportError(error);
      }
    }
    for (const commit of commits) {
      try {
        commit();
      } catch (error: unknown) {
        this.reportError(error);
      }
    }
  };
}
