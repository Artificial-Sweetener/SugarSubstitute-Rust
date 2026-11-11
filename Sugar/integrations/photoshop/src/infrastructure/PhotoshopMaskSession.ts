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
/** Retain a bounded active-selection association across panel reloads without writing document history. */
import type { CanvasMaskReference } from '../../../contracts/CanvasMasksProtocol.generated.js';
import { isCanvasMaskMessage } from '../../../shared/CanvasMasksValidation.generated.js';

/** Qualify a retained selection by the exact native history that still contains its current coverage. */
export interface PhotoshopMaskEditingState {
  readonly mask: CanvasMaskReference;
  readonly revision: string;
}

/** Keep native selection state separate from its plugin-owned persistence boundary. */
export interface PhotoshopMaskSessionPort {
  read(): PhotoshopMaskEditingState | null;
  write(state: PhotoshopMaskEditingState | null): void;
}

declare const localStorage: {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
};
const key = 'sugarcubes.photoshop.active-mask';

/** Persist only identity and revision; Photoshop remains the owner of selection pixels. */
export class PhotoshopMaskSession implements PhotoshopMaskSessionPort {
  /** Report storage failures without mutating a user's document as a fallback. */
  constructor(private readonly report: (error: unknown) => void) {}
  /** Validate cached references through the same authoritative schema as incoming mask commands. */
  read(): PhotoshopMaskEditingState | null {
    try {
      const text = localStorage.getItem(key);
      if (!text) return null;
      if (text.length > 8192) throw new Error('The retained mask association exceeds its budget.');
      const value: unknown = JSON.parse(text);
      if (!value || typeof value !== 'object' || !('mask' in value) || !('revision' in value))
        throw new Error('The retained mask association is invalid.');
      const request: unknown = {
        protocol: 'sugarcubes.canvas.masks',
        version: 2,
        session: '00000000-0000-4000-8000-000000000001',
        requestId: 1,
        command: { type: 'activate', mask: value.mask },
      };
      if (
        !isCanvasMaskMessage(request) ||
        !('command' in request) ||
        request.command.type !== 'activate' ||
        typeof value.revision !== 'string' ||
        !value.revision.length ||
        value.revision.length > 128
      )
        throw new Error('The retained mask identity or revision is invalid.');
      return { mask: request.command.mask, revision: value.revision };
    } catch (error: unknown) {
      this.report(error);
      return null;
    }
  }
  /** Retire released bindings and retain no mask pixels in plugin storage. */
  write(state: PhotoshopMaskEditingState | null): void {
    try {
      if (state) localStorage.setItem(key, JSON.stringify(state));
      else localStorage.removeItem(key);
    } catch (error: unknown) {
      this.report(error);
    }
  }
}
