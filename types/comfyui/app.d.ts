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
/** Describe the ComfyUI application module consumed by the extension. */

import type { ComfyGraph, ComfyNode } from '../../web/comfyui/ui/types/graph.js';

export interface ComfyNodeConstructor {
  prototype: ComfyNode;
}

export interface ComfyNodeDefinition {
  name?: string;
  [key: string]: unknown;
}

export interface ComfyExtension {
  name: string;
  setup?(): void | Promise<void>;
  beforeRegisterNodeDef?(nodeType: ComfyNodeConstructor, nodeData: ComfyNodeDefinition): void;
  nodeCreated?(node: ComfyNode): void;
  loadedGraphNode?(node: ComfyNode): void;
  beforeConfigureGraph?(): void;
  afterConfigureGraph?(missingNodeTypes: string[], app: ComfyApp): void;
}

export interface ComfyApp {
  graph: ComfyGraph;
  registerExtension(extension: ComfyExtension): void;
}

export const app: ComfyApp;
