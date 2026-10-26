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
/** Preserve ordered source identity independently of file transport, native editing and preview presentation. */
import type { CanvasInputSource } from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import type { MaskEditSettings } from './MaskEditSettings.js';
/** Keep ordered source identity separate from its reusable input source. */
export interface InputEntry {
  readonly id: string;
  readonly label: string;
  readonly source: CanvasInputSource;
}
/** Persist one loader's ordered sources and selected entry in the owning workflow node. */
export interface InputBinding {
  readonly version: 2;
  readonly field: string;
  readonly selected: string | null;
  readonly entries: readonly InputEntry[];
  /** Keep file-picker intent distinct from a collection's canvas-owned entries. */
  readonly files: readonly string[];
  /** Preserve optional preparation intent alongside its owning mask loader's sources. */
  readonly edits?: MaskEditSettings;
}
/** Identify durable source associations on their owning workflow node. */
export const MASK_BINDING_KEY = 'sugarcubes_canvas_masks';
/** Identify image-field bindings while retaining the public mask workflow format. */
export const IMAGE_BINDING_KEY = 'sugarcubes_canvas_images';
/** Keep selection attached to identity while moving entries through an ordered batch. */
export function moveInputEntry(binding: InputBinding, id: string, delta: number): InputBinding {
  const entries = [...binding.entries];
  const index = entries.findIndex((entry) => entry.id === id),
    next = index + delta;
  if (index < 0 || next < 0 || next >= entries.length) return binding;
  const value = entries[index];
  const other = entries[next];
  if (!value || !other) return binding;
  entries[index] = other;
  entries[next] = value;
  return { ...binding, entries };
}
