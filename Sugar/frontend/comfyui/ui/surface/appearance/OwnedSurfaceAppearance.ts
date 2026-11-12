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
/** Adapt the published design-system choice and host palette for independently owned controls. */
import type { AppearanceSnapshot, AppearanceSource } from './AppearanceState.js';
import { readDocumentVisualStyle, subscribeVisualStyle } from './VisualStyleBoundary.js';

/** Keep owned editors themed when the native renderer suspends its card styling. */
export class OwnedSurfaceAppearance implements AppearanceSource {
  /** Borrow the existing style and palette owners without retaining a second preference. */
  constructor(
    private readonly document: Document,
    private readonly native: AppearanceSource,
  ) {}

  /** Read selected appearance independently of native-card renderer support. */
  get appearance(): AppearanceSnapshot {
    return { style: readDocumentVisualStyle(this.document), dark: this.native.appearance.dark };
  }

  /** Release both authoritative appearance subscriptions with the owning surface. */
  subscribeAppearance(listener: () => void): () => void {
    const releaseStyle = subscribeVisualStyle(this.document, listener);
    const releasePalette = this.native.subscribeAppearance(listener);
    return () => {
      releaseStyle();
      releasePalette();
    };
  }
}
