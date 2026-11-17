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
/** Reach ComfyUI's own rule for the next value of a controlled number. */
import { isRecord } from '../../types/common.js';
import type { SeedStep } from '../domain/SharedSeedAdvancement.js';

/**
 * Return ComfyUI's randomize, increment and decrement as one step function.
 *
 * ComfyUI publishes the function its "control after generate" widgets use. A shared seed has no
 * such widget, so it calls the same function with the shared range.
 */
export function comfyValueControl(host: unknown = globalThis): SeedStep {
  return (domain, current, mode) => {
    const api = isRecord(host) && isRecord(host.comfyAPI) ? host.comfyAPI.valueControl : null;
    const compute = isRecord(api) ? api.computeNextControlledValue : null;
    if (typeof compute !== 'function') throw new Error('ComfyUI value control is unavailable.');
    const next: unknown = compute.call(
      api,
      {
        type: 'number',
        value: current,
        options: { min: domain.minimum, max: domain.maximum, step2: domain.step },
      },
      mode,
    );
    return typeof next === 'number' && Number.isFinite(next) ? next : current;
  };
}
