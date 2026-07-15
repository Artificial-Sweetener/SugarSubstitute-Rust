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
/** Define ComfyUI graph contracts consumed by SugarCubes. */

import type { Bounds, UnknownRecord, Vec2 } from './common.js';

export type GraphId = string | number;
export type NumericVector = number[] | Float32Array | Float64Array;

export interface ComfyWidget {
  name: string;
  value?: unknown;
  last_value?: unknown;
  options?: { value?: unknown } & UnknownRecord;
  callback?: (...args: unknown[]) => unknown;
  serialize?: boolean;
  type?: string;
  [key: string]: unknown;
}

export interface ComfyInput {
  name?: string;
  type?: unknown;
  link?: GraphId | null;
  links?: GraphId[] | null;
  label?: string;
  dir?: unknown;
  widget?: { name?: string } & UnknownRecord;
  [key: string]: unknown;
}

export interface ComfyOutput {
  name?: string;
  type?: unknown;
  links?: GraphId[] | null;
  link?: GraphId | null;
  label?: string;
  dir?: unknown;
  [key: string]: unknown;
}

export interface ComfyNode {
  id?: GraphId;
  type?: string;
  class_type?: string;
  title?: string;
  pos?: Vec2 | NumericVector;
  size?: Vec2 | NumericVector;
  widgets?: ComfyWidget[];
  inputs?: ComfyInput[];
  outputs?: ComfyOutput[];
  properties?: UnknownRecord;
  widgets_values?: unknown[];
  graph?: ComfyGraph | null;
  getBounding?(): number[];
  getConnectionPos?(isInput: boolean, slot: number, output?: Float32Array): unknown;
  [key: string]: unknown;
}

export interface ComfyGroup {
  id?: GraphId;
  title?: string;
  pos?: Vec2 | NumericVector;
  size?: Vec2 | NumericVector;
  bounding?: Bounds | number[];
  _bounding?: Bounds | NumericVector;
  properties?: UnknownRecord;
  graph?: ComfyGraph;
  [key: string]: unknown;
}

export interface ComfyLink {
  id?: GraphId | null;
  origin_id?: GraphId | null;
  origin_slot?: number | null;
  target_id?: GraphId | null;
  target_slot?: number | null;
  origin?: GraphId | null;
  target?: GraphId | null;
  type?: unknown;
  [key: string]: unknown;
}

export interface ComfyGraph {
  _nodes?: ComfyNode[];
  nodes?: ComfyNode[];
  _groups?: ComfyGroup[];
  groups?: ComfyGroup[];
  links?: Record<string, ComfyLink> | Map<GraphId, ComfyLink> | ComfyLink[];
  _links?: Record<string, ComfyLink> | Map<GraphId, ComfyLink> | ComfyLink[];
  _subgraphs?: Map<GraphId, unknown>;
  canvas?: ComfyCanvas;
  add?(item: ComfyNode | ComfyGroup): void;
  remove?(item: ComfyNode | ComfyGroup): void;
  afterChange?(): void;
  getLink?(id: GraphId): ComfyLink | null;
  setDirtyCanvas?(foreground?: boolean, background?: boolean): void;
  [key: string]: unknown;
}

export interface ComfyCanvas extends UnknownRecord {
  graph?: ComfyGraph;
  canvas?: HTMLCanvasElement;
  selected_nodes?: Record<string, ComfyNode>;
  setDirty?(foreground?: boolean, background?: boolean): void;
  onDrawBackground?: ((ctx: CanvasRenderingContext2D, ...args: unknown[]) => void) | null;
  editor_alpha?: number;
}

export interface ComfyApplication extends UnknownRecord {
  graph?: ComfyGraph;
  canvas?: ComfyCanvas;
  graphToPrompt?(): unknown | Promise<unknown>;
}

export interface ComfyHostApi extends UnknownRecord {
  fetchApi?(path: string, options?: RequestInit): Promise<ApiResponse>;
  queuePrompt?(position: number, payload: unknown): Promise<unknown>;
}

export interface ApiResponse {
  ok?: boolean;
  status?: number;
  statusText?: string;
  json(): Promise<unknown>;
}

export interface SurfaceControl extends UnknownRecord {
  control_id?: string;
  input_name?: string;
  symbol?: string;
  class_type?: string;
}

export interface CubeSurface extends UnknownRecord {
  controls?: SurfaceControl[];
}
