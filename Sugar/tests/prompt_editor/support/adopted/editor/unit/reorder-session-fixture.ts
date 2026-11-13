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
import {
  nativeReorderChipMetadata,
  nativeReorderRootMetadata,
} from './reorder-native-metadata-fixture.ts';
import { createReorderSessionPaintTransport } from './reorder-session-paint-transport.ts';
import { passiveGeometry } from './reorder-passive-geometry-fixture.ts';
/** Finite actual-CM view/scheduling/capture transport for session-owner proof, without native browser certification. */
import { StateEffect, Transaction, type TransactionSpec } from '@codemirror/state';
import { EditorView, WidgetType, type ViewUpdate } from '@codemirror/view';
import { NativeReorderSession } from '../../../../../../frontend/sugarprompt/editor/src/reorder-session.ts';
import {
  reorderPreview,
  reorderPreviewExtension,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { nativeInputSettlement } from '../../../../../../frontend/sugarprompt/editor/src/input-settlement.ts';
import { reorderGrabCursorExtension } from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-cursor.ts';
import { reorderState } from '../transaction/reorder-fixture.ts';
import { sceneErrors } from '../../../../../../frontend/sugarprompt/editor/src/scene-errors.ts';

/** Actual requestMeasure signature is observed without exporting or rebuilding its scheduler. */
type NativeMeasureRequest = NonNullable<Parameters<EditorView['requestMeasure']>[0]>;

/** Explicit external transport boundary observes real transactions without claiming a browser rendering implementation. */
export function fixture(
  settle?: (view: EditorView) => void,
  prepareDocument?: (document: Document) => void,
) {
  let state = reorderState('a, b, c').state;
  const tasks: (() => void)[] = [],
    jobs: NativeMeasureRequest[] = [];
  const original = globalThis.queueMicrotask;
  globalThis.queueMicrotask = (task) => {
    tasks.push(task);
  };
  let updating = false,
    focused = true,
    nextFrame = 0,
    removed = 0;
  let observer: ((transaction: Transaction) => void) | null = null;
  const frames = new Map<number, FrameRequestCallback>(),
    captures = new Set<number>();
  const chipGeometry = new Map<
    number,
    {
      left: number;
      right: number;
      top: number;
      bottom: number;
      width: number;
      height: number;
    }
  >();
  let passive = passiveGeometry;
  const { document: doc, connectRoot } = createReorderSessionPaintTransport({
    passiveGeometry: (node) => passive(node),
    state: () => state,
    line: () => line,
    targets: () => content.querySelectorAll('[data-reorder-target]'),
    chipGeometry,
    request(callback) {
      frames.set(++nextFrame, callback);
      return nextFrame;
    },
    cancel(id) {
      frames.delete(id);
    },
    removed() {
      removed++;
    },
  });
  /** Genuine native line geometry is an explicit finite external boundary; this fixture certifies scheduling, not glyph paint. */
  const line = {
    nodeType: 1,
    localName: 'div',
    className: 'cm-line',
    isConnected: true,
    childNodes: [] as unknown[],
    /** Native class attributes reflect their literal property through source-current stamps. */
    getAttribute(name: string): string | null {
      return name === 'class' ? this.className : null;
    },
    closest: (): object => line,
    parentElement: null,
    getRootNode: () => doc,
    offsetWidth: 100,
    offsetHeight: 20,
    getBoundingClientRect: () => ({
      left: 10,
      right: 110,
      top: 10,
      bottom: 30,
      width: 100,
      height: 20,
    }),
  };
  connectRoot(line);
  /** Native marks retain real declaration identities from the actual field; geometry remains an independent external boundary. */
  function buildTargets(
    published:
      | import('../../../../../../frontend/sugarprompt/editor/src/reorder-preview-contracts.ts').ReorderPreview
      | null,
  ) {
    const declarations = nativeReorderChipMetadata(state);
    return (published?.order.order ?? []).map((id, at) => ({
      nodeType: 1,
      localName: 'span',
      className: declarations.get(String(id))?.className ?? 'sp-reorder-chip',
      childNodes: [],
      /** Expose the actual native mark admission attributes without inventing source ranges. */
      getAttribute(name: string): string | null {
        if (name === 'class') return this.className;
        if (name === 'data-reorder-chip') return this.dataset.reorderChip ?? null;
        return declarations.get(String(id))?.attributes[name] ?? null;
      },
      hasAttribute(name: string): boolean {
        return this.getAttribute(name) !== null;
      },
      parentNode: line,
      parentElement: line,
      ownerDocument: doc,
      /** Native root identity belongs to this exact borrowed document. */
      getRootNode: () => doc,
      /** Publication replacement and teardown remove membership from the finite native line. */
      get isConnected(): boolean {
        return line.isConnected && line.childNodes.includes(this);
      },
      contains(candidate: unknown) {
        return candidate === this;
      },
      dataset: { reorderChip: String(id) } as Record<string, string>,
      closest: () => line,
      /** Plain fixture chips have no nested rich DOM; native tests own actual rich footprint measurement. */
      querySelectorAll() {
        return [];
      },
      getClientRects() {
        return [
          (id === published?.activeChip && published.presentation !== 'pointer'
            ? chipGeometry.get(id)
            : undefined) ?? {
            left: 10 + at * 30,
            right: 30 + at * 30,
            top: 10,
            bottom: 30,
            width: 20,
            height: 20,
          },
        ];
      },
    }));
  }
  let nativeTargets: {
    publication: object;
    nodes: ReturnType<typeof buildTargets>;
  } | null = null;

  const content = {
    className: 'cm-content',
    isConnected: true,
    parentElement: null,
    /** Native class attributes reflect their literal property through source-current stamps. */
    getAttribute(name: string): string | null {
      return name === 'class' ? this.className : null;
    },
    getRootNode: () => doc,
    /** Membership follows actual connected native marks under this finite line, including retirement. */
    contains: (node: object) => node === line || line.childNodes.includes(node),
    setPointerCapture(id: number) {
      captures.add(id);
    },
    hasPointerCapture(id: number) {
      return captures.has(id);
    },
    releasePointerCapture(id: number) {
      captures.delete(id);
    },
    querySelectorAll(selector: string) {
      // This transport supplies target geometry, not complete native line/glyph paint; motion must use its explicit immediate fallback.
      if (selector === '.cm-line') return [];
      const published = state.field(reorderPreview).value;
      const held = selector.match(/\[data-reorder-chip="(\d+)"\]/)?.[1];
      const field = state.field(reorderPreview);
      const nodes =
        field.native && published && nativeTargets?.publication === published
          ? nativeTargets.nodes
          : buildTargets(published);
      if (field.native && published) nativeTargets = { publication: published, nodes };
      line.childNodes = nodes;
      return nodes.filter((node) => held === undefined || node.dataset.reorderChip === held);
    },
    /** Existing geometry-only target lookup supplies the real held identity without claiming browser style or paint. */
    querySelector(selector: string) {
      return this.querySelectorAll(selector)[0] ?? null;
    },
  };
  const view = Object.create(EditorView.prototype) as EditorView;
  Object.defineProperties(view, {
    root: { value: doc },
    /** Supported native position lookup observes this specimen's actual single hard-line boundary. */
    domAtPos: { value: (_position: number) => ({ node: line, offset: 0 }) },
    scaleX: { value: 1 },
    scaleY: { value: 1 },
    state: { get: () => state },
    compositionStarted: { get: () => false },
    hasFocus: { get: () => focused },
    dom: {
      value: {
        ownerDocument: doc,
        /** Observe actual supported root attributes without reconstructing presentation rules. */
        getAttribute(name: string): string | null {
          return nativeReorderRootMetadata(state, view).get(name) ?? null;
        },
      },
    },
    contentDOM: { value: content },
    scrollDOM: {
      value: {
        scrollTop: 0,
        clientHeight: 120,
        getBoundingClientRect() {
          return {
            left: 0,
            right: 200,
            width: 200,
            top: 0,
            bottom: 120,
            height: 120,
          };
        },
      },
    },
  });
  let painted: {
    readonly widget: WidgetType;
    readonly root: HTMLElement;
  } | null = null;
  /** Native draw is the controlled external boundary before each genuine CM measure read, including retained old jobs. */
  let drawing = false;
  const drawWidget = (): void => {
    const preview = state.field(reorderPreview);
    if (!preview.native && nativeTargets) {
      line.childNodes = [];
      nativeTargets = null;
    }
    const widgets: WidgetType[] = [];
    if (!preview.native)
      preview.decorations.between(0, state.doc.length, (_from, _to, value) => {
        if (value.spec.widget instanceof WidgetType) widgets.push(value.spec.widget);
      });
    const next = widgets[0];
    if (!next) {
      painted?.widget.destroy(painted.root);
      painted?.root.remove();
      painted = null;
      return;
    }
    if (painted?.widget.eq(next)) return;
    if (painted && next.updateDOM(painted.root, view, painted.widget)) {
      painted = { widget: next, root: painted.root };
      return;
    }
    const previous = painted;
    const root = next.toDOM(view);
    (root as unknown as { parentNode: unknown }).parentNode = line;
    painted = { widget: next, root };
    previous?.widget.destroy(previous.root);
    previous?.root.remove();
  };
  /** Distinguish CodeMirror-managed widget mounts from independently owned external gesture paint. */
  const draw = (): void => {
    drawing = true;
    try {
      drawWidget();
    } finally {
      drawing = false;
    }
  };
  Object.assign(view, {
    focus() {},
    posAtCoords() {
      return 0;
    },
    requestMeasure(request: NativeMeasureRequest) {
      const next: NativeMeasureRequest = {
          ...request,
          read(nativeView) {
            draw();
            return request.read(nativeView);
          },
        },
        at = jobs.findIndex((job) => job.key === next.key);
      if (at >= 0) jobs[at] = next;
      else jobs.push(next);
    },
    dispatch(...inputs: readonly (Transaction | TransactionSpec)[]) {
      if (updating) throw new Error('nominal-native-update-reentry');
      const tx =
        inputs.length === 1 && inputs[0] instanceof Transaction
          ? inputs[0]
          : state.update(...(inputs as readonly TransactionSpec[]));
      if (tx.startState !== state) throw new Error('nominal-native-continuity');
      state = tx.state;
      updating = true;
      try {
        owner.update({ state, focusChanged: false } as ViewUpdate);
      } finally {
        updating = false;
      }
      const before = tx.startState.field(reorderPreview),
        after = state.field(reorderPreview);
      if (!after.value || (after.value === before.value && after.mask !== before.mask)) draw();
      observer?.(tx);
    },
  });
  // This is an actual extension append; all document/selection/history states above are real CodeMirror states.
  state = state.update({
    effects: StateEffect.appendConfig.of([
      sceneErrors,
      reorderPreviewExtension,
      reorderGrabCursorExtension,
      ...(settle
        ? [
            nativeInputSettlement.of({
              /** Controlled external observation has no composition lifetime. */
              hasCompositionInput: () => false,
              /** No composition descriptor is retained. */
              cancelCompositionInput() {},
              /** No physical input key is synthesized by this state-only fixture. */
              hasHeldKey: () => false,
              /** Execute actual supplied native transactions at the genuine finish boundary. */
              reserveCommand: (view) => ({
                finish: () => settle(view),
                cancel() {},
              }),
            }),
          ]
        : []),
    ]),
  }).state;
  prepareDocument?.(doc as unknown as Document);
  const owner = new NativeReorderSession(view);
  return {
    owner,
    view,
    jobs,
    tasks,
    frames,
    captures,
    /** Real CM invokes plugin geometry/focus notifications while native dispatch remains locked. */
    notifyUpdate(update: ViewUpdate): void {
      if (updating) throw new Error('nominal-native-update-reentry');
      updating = true;
      try {
        owner.update(update);
      } finally {
        updating = false;
      }
    },
    /** External publication/retirement witnesses can certify the actual dispatch-lock frontier. */
    get updating(): boolean {
      return updating;
    },
    /** CM may destroy a failed plugin while the surviving native view still holds its update lock. */
    notifyDestroy(): void {
      if (updating) throw new Error('nominal-native-update-reentry');
      updating = true;
      try {
        owner.destroy();
      } finally {
        updating = false;
      }
    },
    /** Exercise only the genuine WidgetType mount/update callback boundary; this does not claim browser geometry. */
    draw,
    /** External resource observers exclude native widget mount ownership from gesture surfaces. */
    get drawing() {
      return drawing;
    },
    /** Expose actual retained native widget output for owner reuse assertions. */
    get paintedRoot() {
      return painted?.root ?? null;
    },
    /** A proof owner supplies literal external collision viewport operands independently of live publication. */
    setPassiveGeometry(read: typeof passiveGeometry): void {
      passive = read;
    },
    /** Admit an independently specified physical held fragment at the actual synthetic press point before its cold read. */
    setChipGeometry(chip: number, x: number, y: number): void {
      chipGeometry.set(chip, {
        left: x - 5,
        right: x + 15,
        top: y - 10,
        bottom: y + 10,
        width: 20,
        height: 20,
      });
    },
    observe(callback: ((transaction: Transaction) => void) | null) {
      observer = callback;
    },
    /** A foreign native focus transfer is observable independently of the subsequent view update. */
    setFocused(value: boolean): void {
      focused = value;
    },
    /** One native animation frame delivers its current callback batch; cancellation/currentness can retire later callbacks before delivery. */
    frame(time: number) {
      const issued = [...frames];
      if (!issued.length) throw new Error('fixture-missing-frame');
      const causes: unknown[] = [];
      for (const [handle, callback] of issued) {
        if (frames.get(handle) !== callback) continue;
        frames.delete(handle);
        try {
          callback(time);
        } catch (cause) {
          causes.push(cause);
        }
      }
      if (causes.length === 1) throw causes[0];
      if (causes.length) throw new AggregateError(causes, 'fixture-animation-frame-failed');
    },
    get state() {
      return state;
    },
    get removed() {
      return removed;
    },
    measure() {
      const job = jobs.shift();
      if (!job) throw new Error('fixture-missing-measure');
      const value = job.read(view);
      job.write?.(value, view);
    },
    /** Pinned CM reads the current finite request batch before executing its writes in one native measure frame. */
    measureFrame() {
      const requests = jobs.splice(0),
        values = requests.map((job) => job.read(view));
      if (!requests.length) throw new Error('fixture-missing-measure');
      requests.forEach((job, at) => job.write?.(values[at], view));
    },
    task() {
      const task = tasks.shift();
      if (!task) throw new Error('fixture-missing-task');
      task();
    },
    /** Settle every queued owner retirement before removing the native transport and restoring its checkpoint boundary. */
    destroy() {
      const failures: unknown[] = [];
      try {
        owner.destroy();
      } catch (cause) {
        failures.push(cause);
      }
      for (let delivered = 0; delivered < 128 && tasks.length; delivered++) {
        try {
          tasks.shift()!();
        } catch (cause) {
          failures.push(cause);
        }
      }
      if (tasks.length) failures.push(new Error('fixture-retirement-checkpoint-overflow'));
      for (const release of [
        () => painted?.widget.destroy(painted.root),
        () => painted?.root.remove(),
      ]) {
        try {
          release();
        } catch (cause) {
          failures.push(cause);
        }
      }
      painted = null;
      line.isConnected = false;
      line.childNodes = [];
      nativeTargets = null;
      globalThis.queueMicrotask = original;
      if (failures.length === 1) throw failures[0];
      if (failures.length) throw new AggregateError(failures, 'fixture-session-retirement-failed');
    },
  };
}
/** Only actual observed native event fields cross this nominal boundary. */
export function key(value: string): KeyboardEvent {
  return {
    key: value,
    ctrlKey: false,
    metaKey: false,
    getModifierState: () => false,
  } as unknown as KeyboardEvent;
}
/** Explicit primary-pointer specimen does not represent native pointer geometry proof. */
export function pointer(id = 1, y = 20, x = 15): PointerEvent {
  return {
    pointerId: id,
    isPrimary: true,
    button: 0,
    altKey: true,
    ctrlKey: false,
    metaKey: false,
    clientX: x,
    clientY: y,
  } as PointerEvent;
}

/** Cross the independently evidenced ten-pixel donor threshold, then return to the requested actual placement before measuring. */
export function dragPointer(owner: NativeReorderSession, event: PointerEvent = pointer()): void {
  // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
  owner.pointermove({ ...event, clientX: event.clientX + 10 });
  owner.pointermove(event);
}
