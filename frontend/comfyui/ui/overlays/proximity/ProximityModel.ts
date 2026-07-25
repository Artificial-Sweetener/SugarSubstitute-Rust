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
/** Define renderer-neutral proximity endpoints and executable prompt routing. */

import type { UnknownRecord, Vec2 } from '../../types/common.js';
import type { ComfyNode, GraphId } from '../../types/graph.js';

export interface ProximityPromptTarget {
  nodeId: GraphId;
  inputSlot: number;
  inputName: string;
}

interface ProximityEndpoint {
  key: string;
  node?: ComfyNode;
  endpointId: GraphId;
  slot: number;
  cube: unknown;
  instanceId: unknown;
  alias: string;
  type: unknown;
  slotPos: Vec2;
  slotName: string;
}

export interface ProximityOutputEndpoint extends ProximityEndpoint {
  originId: GraphId;
  originSlot: number;
}

export interface ProximityInputEndpoint extends ProximityEndpoint {
  promptTargets: ProximityPromptTarget[];
}

export interface ProximityEndpointSet {
  outputs: ProximityOutputEndpoint[];
  inputs: ProximityInputEndpoint[];
}

export interface ProximityEndpointSource {
  discover(graph: unknown): ProximityEndpointSet;
}

export interface ProximityMatch extends UnknownRecord {
  outputId: GraphId | undefined;
  outputCube?: unknown;
  outputSlot: number;
  outputNode?: ComfyNode;
  outputPos: Vec2;
  outputType?: unknown;
  inputId: GraphId | undefined;
  inputCube?: unknown;
  inputSlot: number;
  inputName: string;
  inputNode?: ComfyNode;
  inputPos: Vec2;
  inputType?: unknown;
  originId: GraphId;
  originSlot: number;
  promptTargets: ProximityPromptTarget[];
  distance: number;
}
