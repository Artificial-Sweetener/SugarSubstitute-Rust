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
/** Describe host globals exposed by ComfyUI and LiteGraph. */

type SugarCubesPublicApi = ReturnType<
  (typeof import('../../web/comfyui/ui/index.js'))['createPublicApi']
>;

interface SugarCubesDebugApi {
  getDirtyState(instanceId: unknown): unknown;
  [key: string]: unknown;
}

interface LiteGraphHost {
  NODE_SLOT_HEIGHT: number;
  NODE_TITLE_HEIGHT: number;
  NODE_COLLAPSED_WIDTH?: number;
  NODE_TEXT_SIZE?: number;
  NODE_FONT?: string;
  GROUP_FONT?: string;
  LGraphCanvas?: {
    node_colors?: Record<string, { color?: string; bgcolor?: string }>;
    prototype: Record<string, unknown>;
  };
  LGraphNode?: { prototype: Record<string, unknown> };
  isValidConnection?(outputType: string, inputType: string): boolean;
  LinkDirection?: { RIGHT?: number; LEFT?: number };
  LinkMarkerShape?: { None?: number };
  EVENT?: unknown;
  EVENT_LINK_COLOR?: string;
  ContextMenu?: new (items: unknown[], options?: { event?: Event }) => unknown;
  LGraphGroup?: {
    new (title?: string): {
      id?: string | number;
      title?: string;
      pos?: number[];
      size?: number[];
      properties?: Record<string, unknown>;
    };
    padding?: number;
    prototype: Record<string, unknown>;
  };
  INPUT: number;
  OUTPUT: number;
  createNode(type: string): object | null;
  registerNodeType(type: string, nodeType: object): void;
}

declare var LiteGraph: LiteGraphHost;

interface Window {
  app?: unknown;
  api?: unknown;
  LiteGraph?: LiteGraphHost;
  LGraphCanvas?: unknown;
  comfyAPI?: {
    vueApp?: {
      config?: {
        globalProperties?: { $toast?: unknown };
      };
    };
  };
  SugarCubes: SugarCubesPublicApi;
  SugarCubesDebug?: SugarCubesDebugApi;
}
