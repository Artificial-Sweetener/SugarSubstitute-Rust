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
 * Define Instance manager collaboration boundaries.
 */

import type { CubeInstance } from './InstanceBuilder.js';
import type { ComfyGraph, ComfyGroup } from '../types/graph.js';

export interface ManagedGroup extends ComfyGroup {
  __sugarcubes_imported?: boolean;
}
export interface InstanceAdapter {
  getConsole?(): Pick<Console, 'warn'> | null;
  getLiteGraph?(): { LGraphGroup?: new (title?: string) => ManagedGroup } | null;
}
export interface InstanceEvents {
  emit?(event: string, payload: unknown): unknown;
}
export interface InstanceScheduler {
  raf?(callback: FrameRequestCallback): number | null;
}
export interface InstanceBuilderTarget {
  build(graph: ComfyGraph | null | undefined): CubeInstance[];
}
export interface InstanceRefreshOptions {
  graph?: ComfyGraph | null | undefined;
  reason?: string | undefined;
  force?: boolean;
}
export interface InstanceMatch {
  instance: CubeInstance;
  group: ManagedGroup | null;
  order: number;
}
export interface InstanceManagerOptions {
  adapter: InstanceAdapter;
  events?: InstanceEvents | null;
  scheduler?: InstanceScheduler | null;
  instanceBuilder?: InstanceBuilderTarget | null;
  requestDirtyRefresh?: ((options: InstanceRefreshOptions) => unknown) | null;
}
