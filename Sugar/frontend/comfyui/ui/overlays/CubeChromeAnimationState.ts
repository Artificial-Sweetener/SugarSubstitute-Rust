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
/**
 * Own transient animation state for rendered Cube chrome instances.
 */

import type { ChromeInstanceState } from './CubeChromeContracts.js';

/** Track transient dirty-state animations by Cube instance. */
export class CubeChromeAnimationState {
  private readonly instances = new Map<string, ChromeInstanceState>();

  clear(): void {
    this.instances.clear();
  }

  isAnimating(state: ChromeInstanceState | null, now: number): boolean {
    return Boolean(state?.appearAt && now - state.appearAt < 1000);
  }

  ensure(instanceId: string): ChromeInstanceState {
    if (!instanceId) return { dirty: false, appearAt: 0 };
    const existing = this.instances.get(instanceId);
    if (existing) return existing;
    const state = { dirty: false, appearAt: 0 };
    this.instances.set(instanceId, state);
    return state;
  }
}
