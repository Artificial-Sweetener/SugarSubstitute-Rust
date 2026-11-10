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
/** Exercise real prompt mounting and renderer-specific surfaces in an isolated browser document. */
import { EditorSelection } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { CubePromptEditorHost } from '../../../frontend/comfyui/ui/prompt_editor/CubePromptEditorHost.js';
import { CubePromptTextareaScope } from '../../../frontend/comfyui/ui/surface/CubePromptTextareaScope.js';
import { ComfyLiteGraphCubeDomWidgetHost } from '../../../frontend/comfyui/ui/surface/ComfyLiteGraphCubeDomWidgetHost.js';
import type { CubeCanvasLayout } from '../../../frontend/comfyui/ui/surface/CubeCanvasLayout.js';
import type { ComfyNode, ComfyWidget } from '../../../frontend/comfyui/ui/types/graph.js';
import { promptCatalogFixture } from './PromptCatalogFixture.js';
import type { ModelCatalogSnapshot } from '../../../frontend/comfyui/ui/model_library/ModelCatalog.js';
import { NativeReorderAxes } from '../../../frontend/sugarprompt/editor/src/reorder-native-axes.js';
import { AppearanceState } from '../../../frontend/comfyui/ui/surface/appearance/AppearanceState.js';
import { OwnedSurfaceAppearance } from '../../../frontend/comfyui/ui/surface/appearance/OwnedSurfaceAppearance.js';
import { publishVisualStyle } from '../../../frontend/comfyui/ui/surface/appearance/VisualStyleBoundary.js';
import type { VisualStyle } from '../../../frontend/comfyui/ui/surface/appearance/VisualStyle.js';
import { accentThemeCss } from '../../../frontend/comfyui/ui/surface/appearance/AccentThemeStyles.js';
import { cubePromptWidgetHeight } from '../../../frontend/comfyui/ui/surface/CubePromptWidgetHeightStore.js';
import { nativeInteractionCss } from '../../../frontend/comfyui/ui/surface/appearance/NativeInteractionStyles.js';

/** Browser observations expose committed host values and lifecycle counts without private editing access. */
export interface PromptBrowserProof {
  mount(renderer: 'nodes-1' | 'nodes-2', scale: number, source: string, promoted?: boolean): void;
  switchRenderer(renderer: 'nodes-1' | 'nodes-2', scale: number): void;
  replace(source: string): void;
  refresh(): void;
  setReadOnly(readOnly: boolean): void;
  reset(): void;
  addInactiveProjection(connected: boolean): void;
  removeDetachedProjection(): void;
  releaseVisibleProjection(): void;
  updateModelTitle(title: string): void;
  axes(transform: string): { readonly x: number; readonly y: number } | null;
  annotateNativeField(): void;
  theme(style: VisualStyle, dark: boolean): void;
  selection(): { readonly anchor: number; readonly head: number } | null;
  /** Offsets where one visual row ends and the next begins, in the mounted editor's visible rows. */
  wrapBoundaries(): readonly number[];
  /** Select a collapsed caret at `position` on the given side of a soft wrap, as a pointer or arrow key does. */
  placeCaret(position: number, side: -1 | 1): void;
  read(): {
    source: unknown;
    callbacks: number;
    before: number;
    after: number;
    escapedHistoryKeys: number;
    allocation: number | null;
    errors: readonly string[];
  };
}

declare global {
  interface Window {
    promptBrowserProof: PromptBrowserProof;
  }
}

const errors: string[] = [];
const appearance = new AppearanceState();
const accentStyle = document.createElement('style');
document.head.append(accentStyle);
const hostFieldStyle = document.createElement('style');
hostFieldStyle.textContent =
  '.comfy-multiline-input{padding:2px}' +
  '.proof-native-prompt-frame:focus-within{box-shadow:0 0 0 2px rgb(182,58,203)}' +
  nativeInteractionCss();
document.head.append(hostFieldStyle);
let catalogListener: ((snapshot: ModelCatalogSnapshot) => void) | null = null;
const host = new CubePromptEditorHost(
  document,
  {
    load: async () => promptCatalogFixture(),
    watch: (listener) => {
      catalogListener = listener;
      return () => {
        catalogListener = null;
      };
    },
    prioritize: async () => undefined,
  },
  (error) => errors.push(error instanceof Error ? error.message : String(error)),
  new OwnedSurfaceAppearance(document, appearance),
);
host.setup();
const root = document.createElement('main');
document.body.append(root);
let textarea: HTMLTextAreaElement;
let widget: ComfyWidget;
let node: ComfyNode;
let release: (() => void) | null = null;
let reconcile: () => void = () => undefined;
let detached: { root: HTMLElement; scope: CubePromptTextareaScope } | null = null;
let sources: import('../../../frontend/comfyui/ui/surface/NativeNodeCardRenderer.js').NativeNodeWidgetSource[] =
  [];
let callbacks = 0;
let before = 0;
let after = 0;
let escapedHistoryKeys = 0;
document.addEventListener('keydown', (event) => {
  if ((event.ctrlKey || event.metaKey) && ['z', 'y'].includes(event.key.toLowerCase()))
    escapedHistoryKeys++;
});
// Reproduce Comfy's capture-phase host contract before native event handlers run.
window.addEventListener(
  'keydown',
  (event) => {
    const active = document.activeElement;
    if (active && Reflect.get(active, 'type') === 'textarea') return;
    if ((event.ctrlKey || event.metaKey) && ['z', 'y'].includes(event.key.toLowerCase()))
      escapedHistoryKeys++;
  },
  true,
);

/** Recreate native presentation while retaining the exact graph/widget lifetime. */
function present(renderer: 'nodes-1' | 'nodes-2', scale: number): void {
  detached?.scope.dispose();
  detached?.root.remove();
  detached = null;
  release?.();
  root.replaceChildren();
  textarea = document.createElement('textarea');
  textarea.value = typeof widget.value === 'string' ? widget.value : '';
  widget.element = textarea;
  root.dataset.renderer = renderer;
  if (renderer === 'nodes-2') {
    const card = document.createElement('div');
    card.className = 'lg-node';
    Object.assign(card.style, {
      width: '720px',
      transform: `translate3d(0,0,0) scale3d(${String(scale)},${String(scale)},${String(scale)})`,
      transformOrigin: 'top left',
    });
    const frame = document.createElement('div');
    frame.dataset.scStyleEditFrame = '';
    frame.className = 'proof-native-prompt-frame';
    frame.append(textarea);
    card.append(frame);
    root.append(card);
    const scope = new CubePromptTextareaScope(card);
    reconcile = () => scope.reconcile(node, sources);
    reconcile();
    release = () => scope.dispose();
  } else {
    const canvas = document.createElement('canvas');
    Object.assign(canvas.style, { width: '1100px', height: '600px' });
    root.append(canvas, textarea);
    const native = new ComfyLiteGraphCubeDomWidgetHost({
      document,
      canvas: { canvas, ds: { scale, offset: [0, 0] } },
    });
    const layout: Pick<CubeCanvasLayout, 'cards'> = {
      cards: [
        {
          node,
          widgetSources: sources,
          id: 'prompt',
          label: 'Positive prompt',
          enabled: true,
          showActivationControl: false,
          activationAction: null,
          rect: { x: 20, y: 20, width: 720, height: 240 },
          bodyHeight: 210,
        },
      ],
    };
    widget.y = 10;
    widget.computedHeight = 180;
    reconcile = () => native.sync([{ layout }]);
    reconcile();
    release = () => native.dispose();
  }
}

/** The mounted prompt's view, found through its content element. */
function promptView(): EditorView {
  const content = document.querySelector<HTMLElement>('[data-sc-prompt-editor] .cm-content');
  const view = content && EditorView.findFromDOM(content);
  if (!view) throw new Error('No mounted prompt editor');
  return view;
}

window.promptBrowserProof = {
  theme(style, dark): void {
    accentStyle.textContent = accentThemeCss('#b63acb', dark);
    publishVisualStyle(document, style);
    appearance.update(style, dark);
  },
  /** Begin one independent workflow with exact stored text and real native callbacks. */
  mount(renderer, scale, source, promoted = false): void {
    release?.();
    release = null;
    host.beforeConfigureGraph();
    callbacks = 0;
    before = 0;
    after = 0;
    escapedHistoryKeys = 0;
    errors.length = 0;
    widget = {
      name: 'text',
      type: 'customtext',
      value: source,
      options: { multiline: true },
      callback: () => {
        callbacks++;
      },
    };
    node = {
      id: 'prompt',
      title: 'Positive prompt',
      type: 'CLIPTextEncode',
      widgets: [widget],
      graph: {
        beforeChange: () => {
          before++;
        },
        afterChange: () => {
          after++;
        },
      },
    };
    sources = [];
    if (promoted) {
      const internal = { ...widget, name: 'text', value: 'internal source' };
      node.widgets = [internal];
      node.inputs = [{ name: 'text', type: 'STRING', widget: { name: 'text' }, link: 1 }];
      const owner: ComfyNode = { id: 'cube', widgets: [widget], graph: node.graph };
      widget.name = 'positive text';
      sources = [{ node: owner, name: widget.name, targetName: 'text' }];
    }
    present(renderer, scale);
  },
  /** Switch only presentation, preserving the existing widget and native editing document. */
  switchRenderer: present,
  /** Simulate workflow/host replacement through the actual widget's public property. */
  replace(source): void {
    widget.value = source;
  },
  refresh: () => reconcile(),
  setReadOnly(readOnly): void {
    textarea.readOnly = readOnly;
    reconcile();
  },
  reset(): void {
    // Comfy always closes a configuration window it opens, retaining or replacing the graph.
    host.beforeConfigureGraph();
    host.afterConfigureGraph();
  },
  /** Prepare a late graph card detached or already connected beneath an inert presentation. */
  addInactiveProjection(connected): void {
    const projection = document.createElement('section');
    projection.className = 'late-native-graph-projection';
    projection.style.visibility = 'hidden';
    projection.inert = true;
    const native = document.createElement('textarea');
    native.value = typeof widget.value === 'string' ? widget.value : '';
    native.readOnly = true;
    projection.append(native);
    const scope = new CubePromptTextareaScope(projection);
    if (connected) document.body.append(projection);
    scope.reconcile(node, sources);
    if (!connected) document.body.append(projection);
    scope.reconcile(node, sources);
    detached = { root: projection, scope };
    reconcile();
  },
  /** Release an inactive native graph projection independently of the visible editor. */
  removeDetachedProjection(): void {
    detached?.scope.dispose();
    detached?.root.remove();
    detached = null;
  },
  /** Return the widget to its surviving graph presentation when the active view closes. */
  releaseVisibleProjection(): void {
    release?.();
    release = null;
    root.replaceChildren();
    if (detached) {
      detached.root.style.visibility = '';
      detached.root.inert = false;
    }
  },
  annotateNativeField(): void {
    const accessory = document.createElement('span');
    accessory.hidden = true;
    textarea.before(accessory);
    reconcile();
  },
  updateModelTitle(title): void {
    const snapshot = promptCatalogFixture();
    catalogListener?.({
      ...snapshot,
      revision: 2,
      items: snapshot.items.map((item) => ({ ...item, title })),
    });
  },
  axes(transform) {
    const node = document.createElement('div');
    node.style.transform = transform;
    root.append(node);
    try {
      return new NativeReorderAxes(window, () => true).read(node);
    } finally {
      node.remove();
    }
  },
  /** Offsets whose upstream and downstream caret rectangles sit on different visual rows. */
  wrapBoundaries() {
    const view = promptView();
    const boundaries: number[] = [];
    for (let position = 1; position < view.state.doc.length; position++) {
      const upstream = view.coordsAtPos(position, -1);
      const downstream = view.coordsAtPos(position, 1);
      if (upstream && downstream && Math.abs(upstream.top - downstream.top) > 8)
        boundaries.push(position);
    }
    return boundaries;
  },
  /** Select one side of a soft wrap through CodeMirror's own selection association. */
  placeCaret(position, side): void {
    promptView().dispatch({
      selection: EditorSelection.create([EditorSelection.cursor(position, side)]),
      scrollIntoView: false,
    });
  },
  /** Observe the mounted editor's public main selection in logical source offsets. */
  selection() {
    const content = document.querySelector<HTMLElement>('[data-sc-prompt-editor] .cm-content');
    const range = content && EditorView.findFromDOM(content)?.state.selection.main;
    return range ? { anchor: range.anchor, head: range.head } : null;
  },
  /** Return host-side evidence; no private CodeMirror state is exposed. */
  read: () => ({
    source: widget.value,
    callbacks,
    before,
    after,
    escapedHistoryKeys,
    errors,
    allocation: cubePromptWidgetHeight(widget),
  }),
};
