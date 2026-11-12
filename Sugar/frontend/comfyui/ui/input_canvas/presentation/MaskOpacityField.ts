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
/** Lease a mask loader's "Visual opacity" percentage field through the themed node-field owner. */
import type { NativeNodeFields, NativeNodeNumberMount } from '../../surface/NativeNodeFields.js';
import type { InputSurfaceCardPort } from '../application/InputEditorPorts.js';
import { WORKFLOW_SURFACE_COPY } from '../application/WorkflowSurfaceCopy.js';

/** Quiet period after the last slider value before the opacity becomes one canvas step. */
const COMMIT_DELAY_MS = 200;

/**
 * Show the field only while the loader has masks on a workflow-surface canvas.
 *
 * Slider drags emit many values; only the value at rest is applied, so one drag becomes one
 * undoable canvas step.
 */
export class MaskOpacityField {
  readonly #stop: () => void;
  #mount: NativeNodeNumberMount | null = null;
  #pending: number | null = null;
  #timer: ReturnType<typeof setTimeout> | null = null;
  #alive = true;

  /** Follow provisioning changes for this loader's card. */
  constructor(
    private readonly surface: InputSurfaceCardPort,
    private readonly fields: NativeNodeFields,
    private readonly label: string,
    private readonly report: (error: unknown) => void,
  ) {
    this.#stop = surface.subscribe(() => this.refresh());
    this.refresh();
  }

  /** Mount or release the field and show the stored percentage. */
  refresh(): void {
    if (!this.#alive) return;
    const opacity = this.surface.opacity();
    if (opacity === null) {
      this.#mount?.dispose();
      this.#mount = null;
      return;
    }
    this.#mount ??= this.fields.mountNumber(
      this.label,
      (value) => this.#change(value),
      { min: 0, max: 100, step: 1, precision: 0, presentation: 'spinner-slider' },
      'mask-visual-opacity',
    );
    this.#mount.update(this.#pending ?? Math.round(opacity * 100));
  }

  /** Apply a pending value immediately and release the field. */
  dispose(): void {
    this.#flush();
    this.#alive = false;
    this.#stop();
    this.#mount?.dispose();
    this.#mount = null;
  }

  /** Hold the newest value until the slider rests. */
  #change(value: number): void {
    this.#pending = Math.min(100, Math.max(0, Math.round(value)));
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = setTimeout(() => this.#flush(), COMMIT_DELAY_MS);
  }

  /** Apply the held value as one change. */
  #flush(): void {
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
    const value = this.#pending;
    this.#pending = null;
    if (value === null || !this.#alive) return;
    try {
      this.surface.changeOpacity(value / 100);
    } catch (error: unknown) {
      this.report(error);
    }
  }
}

/** The field label for one loader, matching multi-loader node conventions. */
export function maskOpacityLabel(field: string | null): string {
  return field
    ? field + ' ' + WORKFLOW_SURFACE_COPY.visualOpacity
    : WORKFLOW_SURFACE_COPY.visualOpacity;
}
