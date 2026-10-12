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
import { WorkflowHostFixture, workflowFixture } from '../../../execution/support/workflow_host.js';
import type { UnknownRecord } from '../../../../frontend/comfyui/ui/types/common.js';
import type {
  ComfyCanvas,
  ComfyGraph,
  ComfyNode,
} from '../../../../frontend/comfyui/ui/types/graph.js';

interface MockNodeConstructor {
  prototype: ComfyNode;
}

export interface MockExtension extends UnknownRecord {
  name?: string;
  addCustomNodeDefs?(definitions: Record<string, UnknownRecord>): void | Promise<void>;
  beforeRegisterVueAppNodeDefs?(definitions: UnknownRecord[]): void;
  registerCustomNodes?(): void | Promise<void>;
  setup?(): void | Promise<void>;
  beforeRegisterNodeDef?(nodeType: MockNodeConstructor, nodeData: UnknownRecord): unknown;
  nodeCreated?(node: ComfyNode): unknown;
}

export interface MockSettingDefinition extends UnknownRecord {
  id: string;
  type: 'boolean' | (() => HTMLElement);
  defaultValue?: unknown;
  onChange?(value: unknown): void;
}

export interface MockSidebarTab extends UnknownRecord {
  title?: string;
  render(container: HTMLElement): unknown;
}

export interface MockExtensionManager extends UnknownRecord {
  workflow: WorkflowHostFixture['extensionManager']['workflow'];
  registerSidebarTab(payload: MockSidebarTab): unknown;
}

export interface MockCanvas extends ComfyCanvas {
  selectedItems: Set<unknown>;
  getNodeMenuOptions(node: unknown): unknown[];
  getCanvasMenuOptions(): unknown[];
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
  loadGraphData(data: unknown): Promise<void>;
  graphToPrompt(): Promise<object>;
  ui?: {
    settings: {
      addSetting(payload: MockSettingDefinition): unknown;
      getSettingValue(id: string, fallback: unknown): unknown;
      setSettingValue(id: string, value: unknown): Promise<void>;
      setSettingValueAsync(id: string, value: unknown): Promise<void>;
    };
  };
  registerExtension(extension: MockExtension): void;
  reset(): void;
}

/** Persist accepted preferences and deliver the same callbacks as the real Comfy settings owner. */
function createMockSettings(): NonNullable<MockApp['ui']>['settings'] {
  const definitions = new Map<string, MockSettingDefinition>();
  const values = new Map<string, unknown>();
  return {
    addSetting: (definition) => definitions.set(definition.id, definition),
    getSettingValue: (id, fallback) => (values.has(id) ? values.get(id) : fallback),
    async setSettingValue(id, value) {
      values.set(id, value);
      definitions.get(id)?.onChange?.(value);
    },
    async setSettingValueAsync(id, value) {
      definitions.get(id)?.onChange?.(value);
      values.set(id, value);
    },
  };
}

/** Reuse the execution contract fixture for the public workflow-store capability. */
function createMockWorkflowStore(): MockExtensionManager['workflow'] {
  return new WorkflowHostFixture(workflowFixture('Test workflow')).extensionManager.workflow;
}

const createMockApp = (): MockApp => ({
  _extensions: [],
  graph: null,
  canvas: {
    selectedItems: new Set(),
    getNodeMenuOptions: () => [],
    getCanvasMenuOptions: () => [],
    onAfterChange: () => undefined,
    onDrawForeground: () => undefined,
    processMouseMove: () => undefined,
    processMouseDown: () => undefined,
    processMouseUp: () => undefined,
    onNodeMoved: () => undefined,
    setDirty: () => undefined,
  },
  extensionManager: { registerSidebarTab: () => undefined, workflow: createMockWorkflowStore() },
  ui: { settings: createMockSettings() },
  clean: () => undefined,
  /** Expose the public async load boundary without configuring a graph in a setup-only fixture. */
  async loadGraphData(_data: unknown) {},
  /** Return an exact public construction object for queue observation. */
  async graphToPrompt() {
    return { workflow: {}, output: {} };
  },
  registerExtension(ext: MockExtension) {
    this._extensions.push(ext);
  },
  reset() {
    this._extensions = [];
    this.graph = null;
    this.canvas = {
      selectedItems: new Set(),
      getNodeMenuOptions: () => [],
      getCanvasMenuOptions: () => [],
      onAfterChange: () => undefined,
      onDrawForeground: () => undefined,
      processMouseMove: () => undefined,
      processMouseDown: () => undefined,
      processMouseUp: () => undefined,
      onNodeMoved: () => undefined,
      setDirty: () => undefined,
    };
    this.extensionManager = {
      registerSidebarTab: () => undefined,
      workflow: createMockWorkflowStore(),
    };
    this.ui = { settings: createMockSettings() };
    this.clean = () => undefined;
    this.loadGraphData = async (_data: unknown) => undefined;
    this.graphToPrompt = async () => ({ workflow: {}, output: {} });
  },
});

const mockHost = globalThis as typeof globalThis & { __sugarCubesMockApp?: MockApp };
export const app = mockHost.__sugarCubesMockApp ?? createMockApp();
mockHost.__sugarCubesMockApp = app;
