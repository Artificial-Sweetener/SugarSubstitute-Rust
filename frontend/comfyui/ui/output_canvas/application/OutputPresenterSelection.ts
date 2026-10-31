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
/** Choose who presents output pixels for this page: the native host or the browser renderer. */

/** The presenter for output pixels; `pending` waits for a host's first native discovery. */
export type OutputPresenter = 'native' | 'browser' | 'pending';

/** Read-only facts the selection consumes; none of them grants canvas or transport authority. */
export interface OutputPresenterPort {
  /** The injected host asked to present output pixels itself. */
  prefersNative(): boolean;
  /** The currently negotiated native output surface, if any. */
  surface(): object | null;
  /** The native canvas session has settled at least one context and surface discovery. */
  discovered(): boolean;
}

/**
 * Fix the page's output presenter from the first conclusive native discovery.
 *
 * A negotiated surface selects native presentation. A host that prefers native presentation
 * keeps the browser renderer unloaded while its discovery is pending; when that discovery
 * settles without a surface (unsupported, unavailable or failed), the browser renderer is
 * selected silently. Either conclusive choice holds for the page lifetime, so a later context
 * refresh never loads the browser renderer beside a working native surface. Without a host
 * preference, the browser renderer presents until a surface is negotiated.
 */
export class OutputPresenterSelection {
  #selected: 'native' | 'browser' | null = null;

  constructor(private readonly port: OutputPresenterPort) {}

  /** Return the selected presenter, fixing it once native discovery is conclusive. */
  current(): OutputPresenter {
    if (this.#selected) return this.#selected;
    if (this.port.surface() !== null) return (this.#selected = 'native');
    if (this.port.discovered()) return (this.#selected = 'browser');
    return this.port.prefersNative() ? 'pending' : 'browser';
  }
}
