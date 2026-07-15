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
import type { UnknownRecord } from '../../../web/comfyui/ui/types/common.js';
import type { ComfyCanvas, ComfyGraph, ComfyNode } from '../../../web/comfyui/ui/types/graph.js';

interface MockNodeConstructor {
  prototype: ComfyNode;
}

export interface MockExtension extends UnknownRecord {
  name?: string;
  setup?(): void | Promise<void>;
  beforeRegisterNodeDef?(nodeType: MockNodeConstructor, nodeData: UnknownRecord): unknown;
  nodeCreated?(node: ComfyNode): unknown;
}

export interface MockSettingDefinition extends UnknownRecord {
  id: string;
  type(): HTMLElement;
}

export interface MockSidebarTab extends UnknownRecord {
  title?: string;
  render(container: HTMLElement): unknown;
}

export interface MockExtensionManager extends UnknownRecord {
  registerSidebarTab(payload: MockSidebarTab): unknown;
}

export interface MockCanvas extends ComfyCanvas {
  onAfterChange(...args: unknown[]): unknown;
  onDrawForeground(...args: unknown[]): unknown;
  processMouseMove(...args: unknown[]): unknown;
  processMouseDown(...args: unknown[]): unknown;
  processMouseUp?(...args: unknown[]): unknown;
  onNodeMoved?(...args: unknown[]): unknown;
  setDirty?(foreground?: boolean, background?: boolean): void;
}

interface MockApp extends UnknownRecord {
  _extensions: MockExtension[];
  graph: ComfyGraph | null;
  canvas: MockCanvas;
  extensionManager: MockExtensionManager;
  clean(): unknown;
  ui?: {
    settings: {
      addSetting(payload: MockSettingDefinition): unknown;
    };
  };
  registerExtension(extension: MockExtension): void;
  reset(): void;
}

const createMockApp = (): MockApp => ({
  _extensions: [],
  graph: null,
  canvas: {
    onAfterChange: () => undefined,
    onDrawForeground: () => undefined,
    processMouseMove: () => undefined,
    processMouseDown: () => undefined,
    processMouseUp: () => undefined,
    onNodeMoved: () => undefined,
    setDirty: () => undefined,
  },
  extensionManager: { registerSidebarTab: () => undefined },
  clean: () => undefined,
  registerExtension(ext: MockExtension) {
    this._extensions.push(ext);
  },
  reset() {
    this._extensions = [];
    this.graph = null;
    this.canvas = {
      onAfterChange: () => undefined,
      onDrawForeground: () => undefined,
      processMouseMove: () => undefined,
      processMouseDown: () => undefined,
      processMouseUp: () => undefined,
      onNodeMoved: () => undefined,
      setDirty: () => undefined,
    };
    this.extensionManager = { registerSidebarTab: () => undefined };
    this.clean = () => undefined;
  },
});

const mockHost = globalThis as typeof globalThis & { __sugarCubesMockApp?: MockApp };
export const app = mockHost.__sugarCubesMockApp ?? createMockApp();
mockHost.__sugarCubesMockApp = app;
