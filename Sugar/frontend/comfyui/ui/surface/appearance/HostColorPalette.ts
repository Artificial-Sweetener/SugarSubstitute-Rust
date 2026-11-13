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
/** Make Comfy's color palette follow the connected desktop host's theme for this document only. */
import { isRecord, type UnknownRecord } from '../../types/common.js';

/** Comfy's persisted palette setting; this owner changes only the page's in-memory value of it. */
const PALETTE_SETTING = 'Comfy.ColorPalette';
/** Comfy ships exactly one dark and one light palette under these identifiers. */
const SHIPPED_PALETTE = { dark: 'dark', light: 'light' } as const;
/** Comfy puts this class on the root for every dark palette and removes it for every light one. */
const DARK_ROOT_CLASS = 'dark-theme';

/** Let the desktop frame hand the host's theme to Comfy without depending on its palette store. */
export interface HostColorScheme {
  follow(dark: boolean): void;
  release(): void;
}

/**
 * Hold Comfy's palette brightness equal to the connected host's theme, as the Substitute window,
 * its washes and every SugarCubes surface already are. Comfy derives the whole web interface from
 * its palette: root class, color scheme, node and graph colors. The host theme therefore selects
 * the shipped dark or light palette by writing the settings store's in-memory value. Comfy's own
 * watchers load the palette from that value, while the write itself never reaches the server, so
 * the user's saved palette and every browser sharing the backend keep their appearance. A saved
 * palette whose brightness already matches the host is left alone; release returns the saved
 * palette.
 */
export class HostColorPalette implements HostColorScheme {
  #target: boolean | null = null;
  /** The palette Comfy holds that this owner did not choose: the user's or the startup value. */
  #saved: unknown = undefined;
  /** The shipped palette this owner last wrote, while its override stands. */
  #applied: string | null = null;
  #unavailableReported = false;

  /** Bind the page and its Comfy host; nothing changes until the host theme is accepted. */
  constructor(
    private readonly document: Document,
    private readonly app: unknown,
    private readonly report: (error: unknown) => void,
  ) {}

  /** Accept the host's theme, which stays authoritative until release. */
  follow(dark: boolean): void {
    this.#target = dark;
    this.reconcile();
  }

  /** Re-assert the host theme after any Comfy palette change, which can come from the user. */
  reconcile(): void {
    const target = this.#target;
    if (target === null) return;
    const values = this.#values();
    if (!values) {
      if (!this.#unavailableReported)
        this.report(new Error('Comfy color palette settings are unavailable for the host theme.'));
      this.#unavailableReported = true;
      return;
    }
    const current = values[PALETTE_SETTING];
    if (current !== this.#applied) this.#saved = current;
    if (this.document.documentElement.classList.contains(DARK_ROOT_CLASS) === target) return;
    const palette = target ? SHIPPED_PALETTE.dark : SHIPPED_PALETTE.light;
    values[PALETTE_SETTING] = palette;
    this.#applied = palette;
  }

  /** Return the palette Comfy held before the override and stop following the host. */
  release(): void {
    const values = this.#target === null ? null : this.#values();
    if (values && this.#applied !== null && values[PALETTE_SETTING] === this.#applied) {
      if (this.#saved === undefined) Reflect.deleteProperty(values, PALETTE_SETTING);
      else values[PALETTE_SETTING] = this.#saved;
    }
    this.#target = null;
    this.#applied = null;
    this.#saved = undefined;
  }

  /** Narrow Comfy's dynamic settings store to its in-memory value table. */
  #values(): UnknownRecord | null {
    const app = isRecord(this.app) ? this.app : {};
    const ui = isRecord(app.ui) ? app.ui : {};
    const settings = isRecord(ui.settings) ? ui.settings : {};
    return isRecord(settings.settingsValues) ? settings.settingsValues : null;
  }
}
