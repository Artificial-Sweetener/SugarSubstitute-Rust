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
 * Model completion proof uses real native state, catalogs and the shipped completion extension;
 * only the host gallery, the DOM document and CodeMirror's tooltip/measure/key transport are doubles.
 */
import {
  EditorState,
  Text,
  Transaction,
  type Extension,
  type TransactionSpec,
} from '@codemirror/state';
import { history } from '@codemirror/commands';
import {
  keymap,
  showTooltip,
  type EditorView,
  type Tooltip,
  type TooltipView,
  type ViewUpdate,
} from '@codemirror/view';
import { encodeSource } from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import {
  createLoraCatalogAdapter,
  prepareTagCompletionCatalog,
  type LoraCatalogItem,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import { exactSourceExtension } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { authorshipExtension } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  semanticStructures,
  installSemanticStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  completionSession,
  installCompletion,
} from '../../../../../../frontend/sugarprompt/editor/src/completion-session.ts';
import {
  loraCompletionCatalog,
  tagCompletionCatalog,
  type PreparedCompletionResult,
} from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-model.ts';
import {
  modelCompletionPopupHost,
  type ModelCompletionArrow,
  type ModelCompletionPopup,
  type ModelCompletionPopupEvents,
  type ModelCompletionPopupHost,
} from '../../../../../../frontend/sugarprompt/editor/src/model-completion-popup.ts';
import { nativeFocusHandoff } from '../../../../../../frontend/sugarprompt/editor/src/focus-handoff.ts';
import { prepareCompletionOptions } from '../../../../../../frontend/sugarprompt/editor/src/completion-options.ts';
import { promptCompletions } from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion.ts';
import { CompletionDocument, CompletionNode } from '../unit/completion-dom-fixture.ts';

/** One literal installed model; folder, file and friendly title are independent spellings. */
export interface ModelRow {
  readonly promptName: string;
  readonly backendValue?: string;
  readonly basename?: string;
  readonly title?: string;
  readonly subtitle?: string | null;
  readonly triggerWords?: readonly string[];
}

/** Build a canonical catalog item; omitted spellings derive from the prompt name the way installed files do. */
export function modelItem(row: ModelRow): LoraCatalogItem {
  const basename = row.basename ?? row.promptName.split('/').at(-1)!;
  return {
    model: {
      promptName: row.promptName,
      backendValue: row.backendValue ?? `${row.promptName}.safetensors`,
      basename,
    },
    title: row.title ?? basename,
    subtitle: row.subtitle ?? null,
    triggerWords: row.triggerWords ?? [],
    thumbnail: { status: 'unavailable' },
    modelPageUrl: null,
  };
}

/** Prepare the real host LoRA completion catalog for literal rows. */
export function modelCatalog(rows: readonly ModelRow[]) {
  const adapter = createLoraCatalogAdapter(
    { revision: 'model-completion', authoritative: true, items: rows.map(modelItem) },
    { searchMetadata: [] },
  );
  return adapter.completionCatalog!;
}

/** One `update` the gallery received. */
export interface PopupUpdate {
  readonly models: readonly string[];
  readonly selected: number;
}

/** A gallery double that records what the engine shows and lays candidates out in a fixed grid. */
export class FakeModelPopup implements ModelCompletionPopup {
  readonly element: HTMLElement;
  readonly node: CompletionNode;
  readonly updates: PopupUpdate[] = [];
  readonly neighborCalls: { selected: number; key: ModelCompletionArrow }[] = [];
  disposed = 0;
  private count = 0;
  private readonly columns: number;
  private readonly events: ModelCompletionPopupEvents;

  /** Retain the engine's event sink so the test can play the person's pointer choice. */
  constructor(document: CompletionDocument, events: ModelCompletionPopupEvents, columns: number) {
    this.node = document.createElement();
    this.node.className = 'host-gallery';
    this.element = this.node as unknown as HTMLElement;
    this.events = events;
    this.columns = columns;
  }

  /** Record the ranked prompt names and current candidate. */
  update(models: readonly LoraCatalogItem[], selected: number): void {
    this.count = models.length;
    this.updates.push({ models: models.map((item) => item.model.promptName), selected });
  }

  /** Move within a `columns`-wide grid; an edge returns the same index. */
  neighbor(selected: number, key: ModelCompletionArrow): number {
    this.neighborCalls.push({ selected, key });
    const column = selected % this.columns;
    const next =
      key === 'ArrowLeft'
        ? column > 0
          ? selected - 1
          : selected
        : key === 'ArrowRight'
          ? column < this.columns - 1 && selected + 1 < this.count
            ? selected + 1
            : selected
          : key === 'ArrowUp'
            ? selected - this.columns
            : selected + this.columns;
    return next >= 0 && next < this.count ? next : selected;
  }

  /** Count releases; the engine must release each mounted gallery exactly once. */
  dispose(): void {
    this.disposed++;
  }

  /** Play the person choosing the candidate at this ranked position. */
  activate(index: number): void {
    this.events.activate(index);
  }

  /** The latest update the engine sent. */
  last(): PopupUpdate | undefined {
    return this.updates.at(-1);
  }
}

/** A host double that mounts one recorded gallery per completion lifetime. */
export class FakeModelPopupHost implements ModelCompletionPopupHost {
  readonly popups: FakeModelPopup[] = [];
  private readonly document: CompletionDocument;
  private readonly columns: number;

  /** Galleries are built from the fixture's document so the engine can append them. */
  constructor(document: CompletionDocument, columns = 3) {
    this.document = document;
    this.columns = columns;
  }

  /** Mount and record a gallery. */
  mount(events: ModelCompletionPopupEvents): ModelCompletionPopup {
    const popup = new FakeModelPopup(this.document, events, this.columns);
    this.popups.push(popup);
    return popup;
  }
}

/** Options for one native model-completion state. */
export interface ModelStateOptions {
  readonly rows?: readonly ModelRow[];
  readonly tags?: readonly string[];
  readonly host?: ModelCompletionPopupHost | null;
  readonly extensions?: Extension;
}

/** Default installed models: two friendly titles and one folder-only path. */
export const defaultModels: readonly ModelRow[] = [
  { promptName: 'folder/midnight', title: 'Midnight' },
  { promptName: 'folder/midday', title: 'Midday' },
  { promptName: 'other/sunrise', title: 'Sunrise' },
];

/** Build state for `source` at `caret` with real catalogs and published native semantics. */
export function modelState(
  source: string,
  caret = encodeSource(source).logical.length,
  options: ModelStateOptions = {},
): EditorState {
  const state = EditorState.create({
    doc: Text.of(encodeSource(source).logical.split('\n')),
    selection: { anchor: caret },
    extensions: [
      exactSourceExtension(source),
      authorshipExtension,
      editorFreshness,
      semanticStructures,
      completionSession,
      history(),
      loraCompletionCatalog.of(modelCatalog(options.rows ?? defaultModels)),
      tagCompletionCatalog.of(
        prepareTagCompletionCatalog({
          revision: 'model-completion',
          bundled: (options.tags ?? ['midnight_sky', 'mid_shot']).map((tag) => ({
            tag,
            popularity: 1,
          })),
          customTags: [],
          censoredTags: [],
        }),
      ),
      modelCompletionPopupHost.of(options.host === undefined ? null : options.host),
      options.extensions ?? [],
    ],
  });
  return publishSemantics(state);
}

/** Publish fresh native semantics, as settled input does before preparation. */
export function publishSemantics(state: EditorState): EditorState {
  return state.update({
    effects: installSemanticStructure.of({
      purpose: 'native',
      prepared: prepareNativeStructure(state),
    }),
  }).state;
}

/** Prepare candidates for a state using a throwaway host, for routing and ranking proof. */
export function modelResult(
  source: string,
  caret?: number,
  options: ModelStateOptions = {},
): PreparedCompletionResult | null {
  const host =
    options.host === undefined ? new FakeModelPopupHost(new CompletionDocument()) : options.host;
  return prepareCompletionOptions(modelState(source, caret, { ...options, host }));
}

/** One native measure request. */
interface MeasureJob {
  readonly key?: unknown;
  readonly read: (view: EditorView) => unknown;
  readonly write?: (value: unknown, view: EditorView) => void;
}

/**
 * Mount the shipped completion extension against a view double.
 *
 * Tooltips are created, updated and destroyed by CodeMirror's own reuse rule (same `create`
 * function reuses the view), keys run the real keymap in precedence order, and focus restoration
 * is counted through the native focus owner facet.
 */
export function mountModelCompletion(source: string, caret?: number, rows?: readonly ModelRow[]) {
  const doc = new CompletionDocument();
  const originals = new Map(
    ['document', 'ResizeObserver'].map((key) => [
      key,
      Object.getOwnPropertyDescriptor(globalThis, key),
    ]),
  );
  Object.defineProperty(globalThis, 'document', { value: doc, configurable: true });
  // The word panel observes its viewport; observation itself publishes nothing here.
  Object.defineProperty(globalThis, 'ResizeObserver', {
    value: class {
      /** Observation has no automatic geometry. */
      observe(): void {}
      /** Nothing to release. */
      disconnect(): void {}
    },
    configurable: true,
  });
  const host = new FakeModelPopupHost(doc);
  let restored = 0;
  const focus = nativeFocusHandoff.of({
    capture: () => ({
      restore: () => {
        restored++;
        return Promise.resolve(true);
      },
      cancel: () => undefined,
    }),
    current: () => true,
    refocus: () => Promise.resolve(true),
  });
  let state = modelState(source, caret, {
    host,
    ...(rows ? { rows } : {}),
    extensions: [promptCompletions, focus],
  });
  const jobs: MeasureJob[] = [];
  let mounted: { create: Tooltip['create']; view: TooltipView }[] = [];
  const view = {
    get state() {
      return state;
    },
    hasFocus: true,
    compositionStarted: false,
    composing: false,
    defaultLineHeight: 19,
    plugin: () => null,
    coordsAtPos: () => ({ left: 90, right: 91, top: 110, bottom: 130 }),
    requestMeasure(job: MeasureJob) {
      jobs.push(job);
    },
    dispatch(spec: TransactionSpec | Transaction) {
      const startState = state;
      state = (spec instanceof Transaction ? spec : state.update(spec)).state;
      sync({ startState, state } as ViewUpdate);
    },
  } as unknown as EditorView;
  /** CodeMirror's tooltip manager: reuse by `create`, create new ones, destroy the rest. */
  function sync(update: ViewUpdate | null): void {
    const wanted = state.facet(showTooltip).filter((tip): tip is Tooltip => tip !== null);
    const next: typeof mounted = [];
    for (const tip of wanted) {
      const known = mounted.find((entry) => entry.create === tip.create);
      if (known) {
        if (update) known.view.update?.(update);
        next.push(known);
      } else {
        const created = { create: tip.create, view: tip.create(view) };
        created.view.mount?.(view);
        next.push(created);
      }
    }
    for (const entry of mounted) if (!next.includes(entry)) entry.view.destroy?.();
    mounted = next;
  }
  /** Run the real keymap for one key in precedence order; true when a binding consumed it. */
  function press(key: string): boolean {
    for (const bindings of state.facet(keymap))
      for (const binding of bindings) if (binding.key === key && binding.run?.(view)) return true;
    return false;
  }
  /** Prepare and install candidates for the current state, as the native preparation owner does. */
  function refresh(): PreparedCompletionResult | null {
    state = publishSemantics(state);
    sync(null);
    const result = prepareCompletionOptions(state);
    view.dispatch({ effects: installCompletion.of(result) });
    return result;
  }
  /** Type text at the caret and refresh candidates. */
  function type(text: string): PreparedCompletionResult | null {
    const head = state.selection.main.head;
    view.dispatch({
      changes: { from: head, insert: text },
      selection: { anchor: head + text.length },
      userEvent: 'input.type',
    });
    return refresh();
  }
  /** Retire every mounted tooltip and restore the replaced globals. */
  function restore(): void {
    for (const entry of mounted) entry.view.destroy?.();
    mounted = [];
    for (const [key, descriptor] of originals)
      if (descriptor) Object.defineProperty(globalThis, key, descriptor);
      else Reflect.deleteProperty(globalThis, key);
  }
  return {
    doc,
    host,
    view,
    jobs,
    press,
    refresh,
    type,
    restore,
    state: () => state,
    tooltips: () => mounted.map((entry) => entry.view),
    restored: () => restored,
  };
}
