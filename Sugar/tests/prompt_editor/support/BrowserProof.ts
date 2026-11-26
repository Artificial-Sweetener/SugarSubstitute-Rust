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
import { WorkerSpellcheckGateway } from '../../../frontend/comfyui/ui/prompt_editor/spellcheck/WorkerSpellcheckGateway.js';
import {
  applyPreferencePatch,
  defaultPreferences,
} from '../../../frontend/sugarprompt/services/src/features/preferences.js';
import type {
  FeaturePreferences,
  PreferencePatch,
} from '../../../frontend/sugarprompt/services/src/features/contracts.js';
import { CubePromptEditorHost } from '../../../frontend/comfyui/ui/prompt_editor/CubePromptEditorHost.js';
import { CubePromptTextareaScope } from '../../../frontend/comfyui/ui/surface/CubePromptTextareaScope.js';
import { ComfyLiteGraphCubeDomWidgetHost } from '../../../frontend/comfyui/ui/surface/ComfyLiteGraphCubeDomWidgetHost.js';
import type { CubeCanvasLayout } from '../../../frontend/comfyui/ui/surface/CubeCanvasLayout.js';
import type { ComfyNode, ComfyWidget } from '../../../frontend/comfyui/ui/types/graph.js';
import { promptCatalogFixture } from './PromptCatalogFixture.js';
import { mountModelGallery } from '../../../frontend/comfyui/ui/model_library/vue/ModelGalleryVueRuntime.js';
import { modelPickerCss } from '../../../frontend/comfyui/ui/model_library/ModelPickerStyles.js';
import { openModelChooser } from '../../../frontend/comfyui/ui/model_library/vue/ModelChooserVueRuntime.js';
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
  /** Mount one prompt; `promptControl` wires its text to an encoder that loads written LoRAs. */
  mount(
    renderer: 'nodes-1' | 'nodes-2',
    scale: number,
    source: string,
    promoted?: boolean,
    promptControl?: boolean,
  ): void;
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
  /** Change the person's prompt editor preferences, as Prompt Editing's switches do. */
  preferences(patch: PreferencePatch): void;
  /** Show a whole file in a full-height pane, as the wildcard and list editor does. */
  mountDocument(
    source: string,
    document: 'wildcard-txt' | 'wildcard-csv' | 'tag-list',
  ): Promise<void>;
  /** The mounted document's text as its editor last reported it. */
  documentText(): string | null;
  /** Every error the page's editors reported. */
  errors(): readonly string[];
  /** Whether a pasted Danbooru link is waiting for its post. */
  booruAsked(): boolean;
  /** Answer the waiting Danbooru lookup with `reply`. */
  answerBooru(reply: unknown): void;
  selection(): { readonly anchor: number; readonly head: number } | null;
  /** Offsets where one visual row ends and the next begins, in the mounted editor's visible rows. */
  wrapBoundaries(): readonly number[];
  /** Select a collapsed caret at `position` on the given side of a soft wrap, as a pointer or arrow key does. */
  placeCaret(position: number, side: -1 | 1): void;
  /** Painted vertical extent of the character starting at position, or null when none is drawn there. */
  characterRow(position: number): { readonly top: number; readonly bottom: number } | null;
  /** CodeMirror's caret rectangle for position on the given side, which the drawn caret must cover. */
  caretRect(
    position: number,
    side: -1 | 1,
  ): { readonly left: number; readonly top: number; readonly bottom: number } | null;
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
// LoRA candidates are drawn by the model library's real wall in its popup surface.
const pickerStyle = document.createElement('style');
pickerStyle.textContent = modelPickerCss;
document.head.append(pickerStyle);
const preferenceListeners = new Set<(preferences: FeaturePreferences) => void>();
// Spellcheck marks are turned on by the tests that prove them, so other proofs see none.
let preferences: FeaturePreferences = applyPreferencePatch(defaultPreferences, {
  allowed: { spellcheck: false },
});
const spellingWords: string[] = [];
let booruAnswer: ((reply: unknown) => void) | null = null;
host.setup(null, {
  app: null,
  logger: { error: () => undefined },
  booruPostLookup: () =>
    new Promise((resolve) => {
      booruAnswer = resolve;
    }),
  spellcheck: new WorkerSpellcheckGateway(
    new Worker('/spellcheck-worker.js', { type: 'module' }),
    {
      module: '/spellcheck/sugarcubes-spellcheck.wasm',
      aff: '/spellcheck/en_US.aff',
      dic: '/spellcheck/en_US.dic',
    },
    {
      read: () => spellingWords,
      add: async (word) => {
        spellingWords.push(word);
      },
    },
    (error) => errors.push(error instanceof Error ? error.message : String(error)),
  ),
  preferences: {
    current: () => preferences,
    subscribe: (listener) => {
      preferenceListeners.add(listener);
      return () => preferenceListeners.delete(listener);
    },
  },
  models: {
    mountGallery: (events) => {
      const element = document.createElement('div');
      element.className = 'sc-model-popup sc-model-popup-inline';
      const gallery = mountModelGallery(element, events);
      return {
        element,
        update: (items, active) => gallery.update(items, active),
        neighbor: (active, key) => gallery.neighbor(active, key),
        dispose: () => {
          gallery.dispose();
          element.remove();
        },
      };
    },
    modelMenu: () => [],
    chooseModel: (kind, label, anchor) =>
      new Promise((resolve) => {
        const items = promptCatalogFixture().items.filter((item) => item.kind === kind);
        openModelChooser(
          document,
          {
            items: { current: () => items, watch: () => () => undefined },
            label,
            anchor,
            context: {
              actions: null,
              updates: null,
              feedback: null,
              detectors: null,
              outputs: null,
              detectorDialog: null,
            },
            session: null,
          },
          resolve,
        );
      }),
  },
});
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

let documentText: string | null = null;
let documentEditor: { dispose(): void } | null = null;

window.promptBrowserProof = {
  async mountDocument(source, kind): Promise<void> {
    documentEditor?.dispose();
    const pane = document.createElement('section');
    pane.dataset.proofDocument = '';
    pane.style.cssText = 'height:420px;width:700px;position:relative';
    document.body.append(pane);
    documentText = source;
    documentEditor = await host.mountDocument(pane, {
      source,
      label: 'Proof file',
      document: kind,
      changed: (text) => {
        documentText = text;
      },
    });
  },
  documentText: () => documentText,
  errors: () => [...errors],
  booruAsked: () => booruAnswer !== null,
  answerBooru(reply): void {
    const answer = booruAnswer;
    booruAnswer = null;
    answer?.(reply);
  },
  preferences(patch): void {
    preferences = applyPreferencePatch(preferences, patch);
    for (const listener of [...preferenceListeners]) listener(preferences);
  },
  theme(style, dark): void {
    accentStyle.textContent = accentThemeCss('#b63acb', dark);
    publishVisualStyle(document, style);
    appearance.update(style, dark);
  },
  /** Begin one independent workflow with exact stored text and real native callbacks. */
  mount(renderer, scale, source, promoted = false, promptControl = false): void {
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
    if (promptControl && node.graph) {
      const encoder: ComfyNode = { id: 'encode', type: 'PCTextEncode', graph: node.graph };
      node.graph._nodes = [node, encoder];
      node.graph.links = [
        { id: 1, origin_id: 'prompt', origin_slot: 0, target_id: 'encode', target_slot: 0 },
      ];
    }
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
  characterRow(position) {
    const rect = promptView().coordsForChar(position);
    return rect ? { top: rect.top, bottom: rect.bottom } : null;
  },
  caretRect(position, side) {
    const rect = promptView().coordsAtPos(position, side);
    return rect ? { left: rect.left, top: rect.top, bottom: rect.bottom } : null;
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
