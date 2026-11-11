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
/** Invalidate native gesture-occlusion measurement when any document content may cover it. */

/** Attributes through which pages and extensions show, hide or move overlays. */
const OVERLAY_ATTRIBUTES = ['style', 'class', 'hidden', 'open', 'popover', 'aria-modal'];

/**
 * Observe every body mutation that can create, show, hide or move an overlay.
 *
 * Third-party popups cannot be enumerated, so this observer deliberately watches the whole body
 * subtree. It only invalidates a measurement: `changed` must coalesce to one measurement per
 * animation frame, and the observer never discovers graphs, surfaces or interaction targets.
 */
export function observeNativeCanvasOverlays(document: Document, changed: () => void): () => void {
  const Mutation = document.defaultView?.MutationObserver;
  const root = document.body ?? document.documentElement;
  if (!Mutation) return () => {};
  const observer = new Mutation(() => changed());
  observer.observe(root, {
    childList: true,
    subtree: true,
    attributes: true,
    attributeFilter: OVERLAY_ATTRIBUTES,
  });
  return () => observer.disconnect();
}
