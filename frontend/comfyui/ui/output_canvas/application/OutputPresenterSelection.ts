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
import type { CanvasDisplayPlan } from './CanvasDisplayPlan.js';
import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';

/** The presenter for output pixels; `pending` waits for a host's first native discovery. */
export type OutputPresenter = 'native' | 'browser' | 'pending';

/**
 * Who presents output images: the host itself, the page, or not yet known while the host has not
 * described itself.
 */
export type OutputPresentationOwner = 'host' | 'page' | 'pending';

/** Read-only facts the selection consumes; none of them grants canvas or transport authority. */
export interface OutputPresenterPort {
  /** Whether the host declared that it presents output images, including unavailable states. */
  owner(): OutputPresentationOwner;
  /** The currently negotiated native output surface, if any. */
  surface(): object | null;
  /** The current native context and surface discovery has settled. */
  discovered(): boolean;
  /** Request the existing browser media viewer for output content; native Input keeps this false. */
  browserMedia?(): boolean;
}

/**
 * Keep desktop image presentation native throughout startup and connection changes.
 *
 * A page whose host presents output waits for discovery, then presents native pixels or native
 * diagnostics, and never falls back to browser images. Until the host describes itself the page
 * waits rather than showing browser images it may have to withdraw. Other hosts retain browser
 * presentation until a native surface is negotiated.
 */
export class OutputPresenterSelection {
  #native = false;

  constructor(private readonly port: OutputPresenterPort) {}

  /** Keep still images native and hand distinct media to its viewer after current discovery. */
  current(): OutputPresenter {
    const available = this.port.surface() !== null;
    if (available) this.#native = true;
    const owner = this.port.owner();
    this.#native ||= owner === 'host';
    if (!this.#native) return owner === 'pending' ? 'pending' : 'browser';
    if (!available && !this.port.discovered()) return 'pending';
    return this.port.browserMedia?.() ? 'browser' : 'native';
  }
}

/** Consume the existing display plan rather than introducing a second selection or grid policy. */
export function canvasRequiresBrowserMedia(plan: CanvasDisplayPlan): boolean {
  const media = (item: WorkflowOutputItem | null | undefined) => {
    if (!item) return false;
    return (
      item.mediaKind === 'animated' || item.mediaKind === 'video' || item.mediaKind === 'model'
    );
  };
  if (plan.view.mode === 'overview') return plan.grid.some((entry) => entry.items.some(media));
  return !plan.preview && (media(plan.primary?.item) || media(plan.secondary?.item));
}
