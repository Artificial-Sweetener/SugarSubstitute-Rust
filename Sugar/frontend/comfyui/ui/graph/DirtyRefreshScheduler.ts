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
 * Own the SugarCubes graph integration layer in `frontend/comfyui/ui/graph/DirtyRefreshScheduler.js`.
 */

import type { ComfyGraph } from '../types/graph.js';

export interface RefreshScheduler {
  raf(callback: FrameRequestCallback): number | null;
}

export interface DirtyRefreshOptions {
  graph?: ComfyGraph | null | undefined;
  reason?: string | undefined;
}

interface DirtyRefreshSchedulerOptions {
  scheduler?: RefreshScheduler | null | undefined;
  onRefresh?: ((options: DirtyRefreshOptions) => void) | null;
}

/**
 * Coordinate dirty refresh scheduler behavior for the SugarCubes UI.
 */
export class DirtyRefreshScheduler {
  private readonly scheduler: RefreshScheduler | null;
  private readonly onRefresh: ((options: DirtyRefreshOptions) => void) | null;
  private pending: boolean;
  private latestOptions: DirtyRefreshOptions | null;

  constructor({ scheduler, onRefresh }: DirtyRefreshSchedulerOptions = {}) {
    this.scheduler = scheduler || null;
    this.onRefresh = typeof onRefresh === 'function' ? onRefresh : null;
    this.pending = false;
    this.latestOptions = null;
  }

  requestRefresh(options: DirtyRefreshOptions = {}): void {
    this.latestOptions = options || {};
    if (this.pending) {
      return;
    }
    this.pending = true;
    this.scheduler?.raf?.(() => {
      this.pending = false;
      const optionsToUse = this.latestOptions ?? {};
      this.latestOptions = null;
      this.onRefresh?.(optionsToUse);
    });
  }
}
