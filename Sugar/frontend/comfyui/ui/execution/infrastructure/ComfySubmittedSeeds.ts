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
/** Decode accepted scalar inputs without reading armed widgets or assuming seed names. */
import { isRecord } from '../../types/common.js';
import type { SubmittedSeed } from '../domain/WorkflowSeed.js';

/** Record integer inputs so renamed seed controls can resolve their exact submitted owner. */
export function decodeComfySubmittedSeeds(prompt: unknown): readonly SubmittedSeed[] {
  if (!isRecord(prompt) || Array.isArray(prompt)) return [];
  return Object.entries(prompt).flatMap(([path, node]) => {
    if (!isRecord(node) || !isRecord(node.inputs)) return [];
    return Object.entries(node.inputs).flatMap(([name, value]) =>
      typeof value === 'number' && Number.isSafeInteger(value) ? [{ path, name, value }] : [],
    );
  });
}
