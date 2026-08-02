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
/** Resolve the installed Comfy runtime modules used by native Nodes 2.0. */

import type { ComfyNode } from '../types/graph.js';
import type { UnknownRecord } from '../types/common.js';
import { isRecord } from '../types/common.js';
import {
  discoverComfyFrontendModules,
  type ComfyFrontendModule,
  type ComfyRuntimeTextFetcher,
} from './ComfyFrontendModuleGraph.js';

const VUE_RUNTIME_URL_PATTERN = /\/vendor-vue-core-[^/?]+\.js(?:\?.*)?$/;
const PRIME_VUE_RUNTIME_URL_PATTERN = /\/vendor-primevue-[^/?]+\.js(?:\?.*)?$/;
const NATIVE_NODE_COMPONENT_PATTERN =
  /([A-Za-z_$][\w$]*)=[A-Za-z_$][\w$]*\(\{__name:[`'"]LGraphNode[`'"]/;
const NODE_DATA_CAPABILITY = 'extractVueNodeData';
const SLOT_LAYOUT_CAPABILITY = 'requestSlotLayoutSyncForAllNodes';
const RELATIVE_MODULE_SPECIFIER_PATTERN = /((?:from|import)\s*(?:\(\s*)?)(["'])\.\/([^"'`]+)\2/g;
const RELATIVE_ASSET_URL_PATTERN = /new URL\((["'`])([^"'`]+)\1,\s*import\.meta\.url\)/g;
const FETCH_TIMEOUT_MS = 5_000;
const NATIVE_NODE_COMPONENT_EXPORT = 'sugarcubesNativeNodeComponent';
const NODE_DATA_FUNCTION_EXPORT = 'sugarcubesExtractVueNodeData';

export interface ComfyNodes2RuntimeAssets {
  entry: string;
  vueRuntime: string;
  nodeDataRuntime: string;
  slotLayoutRuntime: string | null;
  graphView: string;
}

export interface ComfySettingsRuntimeAssets {
  entry: string;
  vueRuntime: string;
  primeVueRuntime: string;
}

interface ComfyVueRuntimeAsset {
  entry: string;
  vueRuntime: string;
}

export interface ComfyVueRenderRuntime {
  render(vnode: unknown, target: HTMLElement): void;
  h(component: unknown, props: UnknownRecord): unknown;
}

export interface ComfyVueNodeRenderRuntime extends ComfyVueRenderRuntime {
  extractVueNodeData(node: ComfyNode): UnknownRecord;
}

export interface ComfyVueRuntime extends ComfyVueNodeRenderRuntime {
  requestSlotLayoutSync(): void;
}

export type RuntimeModuleImporter = (url: string) => Promise<UnknownRecord>;
export type RuntimeTextFetcher = ComfyRuntimeTextFetcher;

/** Report an installed Comfy runtime that no longer matches the guarded contract. */
export class NativeRendererCompatibilityError extends Error {
  /** Create a compatibility failure with actionable boundary context. */
  constructor(message: string) {
    super(message);
    this.name = 'NativeRendererCompatibilityError';
  }
}

/** Discover Nodes 2 assets by their semantic capabilities across Comfy's module graph. */
export async function discoverComfyNodes2RuntimeAssets(
  documentRef: Document,
  fetchText: RuntimeTextFetcher = fetchRuntimeText,
): Promise<ComfyNodes2RuntimeAssets> {
  const discovery = await discoverModules(documentRef, fetchText, [
    { key: 'vue', matches: (module) => VUE_RUNTIME_URL_PATTERN.test(module.url) },
    { key: 'nodeData', matches: exposesNodeDataCapability },
    { key: 'slotLayout', required: false, matches: exposesSlotLayoutCapability },
    { key: 'graphView', matches: exposesNativeNodeComponent },
  ]);
  return {
    entry: discovery.entry,
    vueRuntime: requireModule(discovery.modules, 'vue').url,
    nodeDataRuntime: requireModule(discovery.modules, 'nodeData').url,
    slotLayoutRuntime: discovery.modules.get('slotLayout')?.url ?? null,
    graphView: requireModule(discovery.modules, 'graphView').url,
  };
}

/** Discover Settings renderer assets independently of Nodes 2 internals. */
export async function discoverComfySettingsRuntimeAssets(
  documentRef: Document,
  fetchText: RuntimeTextFetcher = fetchRuntimeText,
): Promise<ComfySettingsRuntimeAssets> {
  const discovery = await discoverModules(documentRef, fetchText, [
    { key: 'vue', matches: (module) => VUE_RUNTIME_URL_PATTERN.test(module.url) },
    { key: 'primeVue', matches: (module) => PRIME_VUE_RUNTIME_URL_PATTERN.test(module.url) },
  ]);
  return {
    entry: discovery.entry,
    vueRuntime: requireModule(discovery.modules, 'vue').url,
    primeVueRuntime: requireModule(discovery.modules, 'primeVue').url,
  };
}

/** Load Comfy's exact LGraphNode component without requiring a root-graph node. */
export async function loadComfyNativeNodeComponent(
  documentRef: Document,
  fetchText: RuntimeTextFetcher = fetchRuntimeText,
  importModule: RuntimeModuleImporter = importRuntimeModule,
): Promise<UnknownRecord> {
  const discovery = await discoverModules(documentRef, fetchText, [
    { key: 'graphView', matches: exposesNativeNodeComponent },
  ]);
  const graphView = requireModule(discovery.modules, 'graphView');
  const moduleSource = buildNativeNodeComponentModule(graphView.source, graphView.url);
  const moduleUrl = URL.createObjectURL(new Blob([moduleSource], { type: 'text/javascript' }));
  try {
    const loaded = await importModule(moduleUrl);
    const component = loaded[NATIVE_NODE_COMPONENT_EXPORT];
    if (!isRecord(component)) {
      throw new NativeRendererCompatibilityError(
        'Comfy native node component export was not an object.',
      );
    }
    return component;
  } finally {
    URL.revokeObjectURL(moduleUrl);
  }
}

/** Load the PrimeVue Select used by Comfy's Settings panel. */
export async function loadComfySettingsSelectComponent(
  documentRef: Document,
  fetchText: RuntimeTextFetcher = fetchRuntimeText,
  importModule: RuntimeModuleImporter = importRuntimeModule,
): Promise<UnknownRecord> {
  return await loadComfyPrimeVueComponent(documentRef, 'Select', fetchText, importModule);
}

/** Load the PrimeVue AutoComplete used by Comfy's Settings component family. */
export async function loadComfySettingsAutoCompleteComponent(
  documentRef: Document,
  fetchText: RuntimeTextFetcher = fetchRuntimeText,
  importModule: RuntimeModuleImporter = importRuntimeModule,
): Promise<UnknownRecord> {
  return await loadComfyPrimeVueComponent(documentRef, 'AutoComplete', fetchText, importModule);
}

/**
 * Expose the installed component while keeping every renderer dependency native.
 *
 * Comfy currently keeps LGraphNode private inside its GraphView chunk. SugarCubes
 * adds one export to that installed module and resolves its relative dependencies
 * against the original asset URL. No node markup, widgets, or styling are copied.
 */
export function buildNativeNodeComponentModule(source: string, graphViewUrl: string): string {
  const componentMatch = source.match(NATIVE_NODE_COMPONENT_PATTERN);
  const componentIdentifier = componentMatch?.[1];
  if (!componentIdentifier) {
    throw new NativeRendererCompatibilityError(
      'Comfy GraphView no longer contains the native LGraphNode component boundary.',
    );
  }
  const resolvedSource = resolveInstalledModuleReferences(source, graphViewUrl);
  return `${resolvedSource}\nexport{${componentIdentifier} as ${NATIVE_NODE_COMPONENT_EXPORT}};\n`;
}

/** Expose Comfy's retained private node-data function from its installed chunk. */
export function buildPrivateNodeDataModule(source: string, moduleUrl: string): string {
  const declarationPattern = new RegExp(`function\\s+${NODE_DATA_CAPABILITY}\\s*\\(`);
  if (!declarationPattern.test(source)) {
    throw new NativeRendererCompatibilityError(
      'Comfy no longer retains the native node-data function boundary.',
    );
  }
  const resolvedSource = resolveInstalledModuleReferences(source, moduleUrl);
  return `${resolvedSource}\nexport{${NODE_DATA_CAPABILITY} as ${NODE_DATA_FUNCTION_EXPORT}};\n`;
}

/** Resolve imports and module-relative assets before executing an installed chunk as a blob. */
function resolveInstalledModuleReferences(source: string, moduleUrl: string): string {
  return source
    .replace(
      RELATIVE_MODULE_SPECIFIER_PATTERN,
      (_match, prefix: string, quote: string, relativePath: string) =>
        `${prefix}${quote}${new URL(relativePath, moduleUrl).href}${quote}`,
    )
    .replace(
      RELATIVE_ASSET_URL_PATTERN,
      (_match, _quote: string, relativePath: string) =>
        `new URL(${JSON.stringify(new URL(relativePath, moduleUrl).href)})`,
    );
}

/** Resolve one named component from Comfy's installed PrimeVue bundle. */
async function loadComfyPrimeVueComponent(
  documentRef: Document,
  componentName: string,
  fetchText: RuntimeTextFetcher,
  importModule: RuntimeModuleImporter,
): Promise<UnknownRecord> {
  const assets = await discoverComfySettingsRuntimeAssets(documentRef, fetchText);
  const loaded = await importModule(assets.primeVueRuntime);
  for (const value of Object.values(loaded)) {
    if (isRecord(value) && (value.name === componentName || value.__name === componentName)) {
      return value;
    }
  }
  throw new NativeRendererCompatibilityError(
    `Comfy PrimeVue runtime no longer exports the ${componentName} component.`,
  );
}

/** Load only Vue's render primitives for non-node Comfy component mounts. */
export async function loadComfyVueRenderRuntime(
  documentRef: Document,
  fetchText: RuntimeTextFetcher = fetchRuntimeText,
  importModule: RuntimeModuleImporter = importRuntimeModule,
): Promise<ComfyVueRenderRuntime> {
  const assets = await discoverComfyVueRuntimeAsset(documentRef, fetchText);
  const vueModule = await importModule(assets.vueRuntime);
  const render = findNamedFunction(vueModule, 'render');
  const h = findNamedFunction(vueModule, 'h');
  if (!render || !h) {
    throw new NativeRendererCompatibilityError(
      'Comfy Vue render capabilities changed; refusing a non-native fallback.',
    );
  }
  return {
    render: (vnode, target) => {
      render(vnode, target);
    },
    h: (component, props) => h(component, props),
  };
}

/** Load and capability-check the actual renderer functions from installed Comfy. */
export async function loadComfyVueRuntime(
  documentRef: Document,
  fetchText: RuntimeTextFetcher = fetchRuntimeText,
  importModule: RuntimeModuleImporter = importRuntimeModule,
): Promise<ComfyVueRuntime> {
  const assets = await discoverComfyNodes2RuntimeAssets(documentRef, fetchText);
  const nodeDataModulePromise = importModule(assets.nodeDataRuntime);
  const [vueModule, nodeDataModule, slotLayoutModule] = await Promise.all([
    importModule(assets.vueRuntime),
    nodeDataModulePromise,
    assets.slotLayoutRuntime === null
      ? Promise.resolve(null)
      : assets.slotLayoutRuntime === assets.nodeDataRuntime
        ? nodeDataModulePromise
        : importModule(assets.slotLayoutRuntime),
  ]);
  const render = findNamedFunction(vueModule, 'render');
  const h = findNamedFunction(vueModule, 'h');
  const extractVueNodeData =
    findNamedFunction(nodeDataModule, NODE_DATA_CAPABILITY) ??
    (await loadPrivateNodeDataFunction(assets.nodeDataRuntime, fetchText, importModule));
  const requestSlotLayoutSyncForAllNodes = findNamedFunction(
    slotLayoutModule ?? {},
    SLOT_LAYOUT_CAPABILITY,
  );
  if (!render || !h || !extractVueNodeData) {
    throw new NativeRendererCompatibilityError(
      'Comfy native renderer capabilities changed; refusing a non-native fallback.',
    );
  }
  return {
    render: (vnode, target) => {
      render(vnode, target);
    },
    h: (component, props) => h(component, props),
    extractVueNodeData: (node) => {
      const value = extractVueNodeData(node);
      if (!isRecord(value)) {
        throw new NativeRendererCompatibilityError(
          'Comfy returned invalid native node render data.',
        );
      }
      return value;
    },
    requestSlotLayoutSync: () => {
      requestSlotLayoutSyncForAllNodes?.();
    },
  };
}

/** Load a retained private node-data function when the chunk does not export it. */
async function loadPrivateNodeDataFunction(
  moduleUrl: string,
  fetchText: RuntimeTextFetcher,
  importModule: RuntimeModuleImporter,
): Promise<((...args: unknown[]) => unknown) | null> {
  const source = await fetchText(moduleUrl);
  const moduleSource = buildPrivateNodeDataModule(source, moduleUrl);
  const transformedUrl = URL.createObjectURL(new Blob([moduleSource], { type: 'text/javascript' }));
  try {
    const loaded = await importModule(transformedUrl);
    const value = loaded[NODE_DATA_FUNCTION_EXPORT];
    if (typeof value !== 'function') return null;
    return (...args: unknown[]) => Reflect.apply(value, undefined, args) as unknown;
  } finally {
    URL.revokeObjectURL(transformedUrl);
  }
}

/** Discover Vue alone so unrelated private capabilities cannot block component rendering. */
async function discoverComfyVueRuntimeAsset(
  documentRef: Document,
  fetchText: RuntimeTextFetcher,
): Promise<ComfyVueRuntimeAsset> {
  const discovery = await discoverModules(documentRef, fetchText, [
    { key: 'vue', matches: (module) => VUE_RUNTIME_URL_PATTERN.test(module.url) },
  ]);
  return {
    entry: discovery.entry,
    vueRuntime: requireModule(discovery.modules, 'vue').url,
  };
}

/** Translate generic graph-discovery failures into the renderer compatibility contract. */
async function discoverModules(
  documentRef: Document,
  fetchText: RuntimeTextFetcher,
  requirements: ReadonlyArray<{
    key: string;
    required?: boolean;
    matches(module: ComfyFrontendModule): boolean;
  }>,
): Promise<{
  entry: string;
  modules: ReadonlyMap<string, ComfyFrontendModule>;
}> {
  try {
    const discovery = await discoverComfyFrontendModules(documentRef, requirements, fetchText);
    const missing = requirements
      .filter((requirement) => requirement.required !== false)
      .map((requirement) => requirement.key)
      .filter((key) => !discovery.modules.has(key));
    if (missing.length > 0) {
      throw new NativeRendererCompatibilityError(
        `Comfy native renderer is missing capabilities: ${missing.join(', ')}.`,
      );
    }
    return discovery;
  } catch (error: unknown) {
    if (error instanceof NativeRendererCompatibilityError) throw error;
    throw new NativeRendererCompatibilityError(
      error instanceof Error ? error.message : 'Comfy frontend module discovery failed.',
    );
  }
}

/** Require one capability match after a successful bounded discovery. */
function requireModule(
  modules: ReadonlyMap<string, ComfyFrontendModule>,
  key: string,
): ComfyFrontendModule {
  const module = modules.get(key);
  if (!module) {
    throw new NativeRendererCompatibilityError(`Comfy runtime capability '${key}' was not found.`);
  }
  return module;
}

/** Recognize the installed owner of Comfy's native node-data conversion. */
function exposesNodeDataCapability(module: ComfyFrontendModule): boolean {
  return module.source.includes(NODE_DATA_CAPABILITY);
}

/** Recognize Comfy's optional global slot-layout coordinator. */
function exposesSlotLayoutCapability(module: ComfyFrontendModule): boolean {
  return module.source.includes(SLOT_LAYOUT_CAPABILITY);
}

/** Recognize GraphView by the native component definition SugarCubes reuses. */
function exposesNativeNodeComponent(module: ComfyFrontendModule): boolean {
  return NATIVE_NODE_COMPONENT_PATTERN.test(module.source);
}

/** Fetch a same-origin runtime module with an explicit timeout. */
async function fetchRuntimeText(url: string): Promise<string> {
  const controller = new AbortController();
  const timeout = window.setTimeout(() => controller.abort(), FETCH_TIMEOUT_MS);
  try {
    const response = await fetch(url, { signal: controller.signal });
    if (!response.ok) {
      throw new NativeRendererCompatibilityError(
        `Comfy runtime asset request failed with status ${response.status}.`,
      );
    }
    return await response.text();
  } finally {
    window.clearTimeout(timeout);
  }
}

/** Import a same-origin installed Comfy module by its discovered URL. */
async function importRuntimeModule(url: string): Promise<UnknownRecord> {
  const loaded: unknown = await import(url);
  if (!isRecord(loaded)) {
    throw new NativeRendererCompatibilityError('Comfy runtime module was not an object.');
  }
  return loaded;
}

/** Resolve a minified ESM export by its retained function name. */
function findNamedFunction(
  module: UnknownRecord,
  expectedName: string,
): ((...args: unknown[]) => unknown) | null {
  for (const value of Object.values(module)) {
    if (typeof value === 'function' && value.name === expectedName) {
      return value as (...args: unknown[]) => unknown;
    }
  }
  return null;
}
