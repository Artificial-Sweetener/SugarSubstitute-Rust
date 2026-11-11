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
/** Actual pointer owner and immutable reorder publications arbitrate one finite external arrow portal. */
import assert from 'node:assert/strict';
import test, { after } from 'node:test';
import { registerHooks } from 'node:module';
import { readFileSync } from 'node:fs';
import { StateEffect, type TransactionSpec } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import type { EditorView, ViewUpdate } from '@codemirror/view';
import { weightStateFor } from '../transaction/weight-command-state.ts';
import { captureReorder } from '../transaction/reorder-fixture.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { installProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import {
  installReorderPreview,
  reorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import {
  reorderGrabCursor,
  setReorderGrabCursor,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-cursor.ts';

const observed = new URL(
  '../../../../../../frontend/sugarprompt/editor/src/weight-pointer.ts',
  import.meta.url,
).href;
const hook = registerHooks({
  /** Observe the unchanged private owner without adding a production export. */
  load(url, context, next) {
    const result = next(url, context);
    return url === observed
      ? {
          ...result,
          source: readFileSync(new URL(url), 'utf8') + '\nexport { PointerWeights };\n',
        }
      : result;
  },
});
after(() => hook.deregister());

/** Only existing owner entry/update/teardown methods cross this observation boundary. */
interface PointerOwner {
  hover(event: MouseEvent): void;
  update(update: ViewUpdate): void;
  destroy(): void;
}
const observedModule = (await import(
  '../../../../../../frontend/sugarprompt/editor/src/weight-pointer.ts'
)) as unknown as {
  PointerWeights: new (view: EditorView) => PointerOwner;
};
type Measure = NonNullable<Parameters<EditorView['requestMeasure']>[0]>;

/** Literal DOM boundary supplies portal/event storage; native proof owns browser hit testing and paint. */
class PortalElement {
  hidden = true;
  className = '';
  type = '';
  style: Record<string, string> = {};
  readonly children: PortalElement[] = [];
  readonly listeners = new Map<string, EventListener>();
  readonly dataset: Record<string, string> = {};
  isConnected = true;
  /** Native attributes need no independent interpretation in the pointer owner proof. */
  setAttribute(_name: string, _value: string): void {}
  /** Literal portal descendants retain their own input listeners. */
  append(child: PortalElement): void {
    this.children.push(child);
  }
  /** Event delivery observes the listener installed by the actual control view. */
  addEventListener(name: string, callback: EventListener): void {
    this.listeners.set(name, callback);
  }
  /** Teardown retires connected portal membership. */
  remove(): void {
    this.isConnected = false;
  }
  /** This canonical specimen is an emphasis number, never a LoRA/wildcard number. */
  matches(_selector: string): boolean {
    return false;
  }
  /** Hover supplies the actual finite source-backed specimen label. */
  closest(_selector: string): this {
    return this;
  }
  /** Geometry belongs to the external measurement transport, not the expected arbitration result. */
  getBoundingClientRect() {
    return { left: 40, right: 60, top: 40, bottom: 60, width: 20, height: 20 };
  }
}
/** Controlled primary arrow input exercises the real portal listener and repeat owner. */
class PrimaryPointer {
  readonly button = 0;
  readonly pointerId = 7;
  readonly clientX = 50;
  readonly clientY = 35;
  /** Native prevention has no source authority. */
  preventDefault(): void {}
}

/** Run a real owner with native immutable state, finite measured callbacks, and primary-first cleanup. */
async function usingPointerOwner(
  proof: (f: ReturnType<typeof pointerFixture>) => void,
): Promise<void> {
  const f = pointerFixture(),
    failures: unknown[] = [];
  try {
    proof(f);
  } catch (cause) {
    failures.push(cause);
  }
  try {
    f.destroy();
  } catch (cause) {
    failures.push(cause);
  }
  if (failures.length === 1) throw failures[0];
  if (failures.length)
    throw new AggregateError(failures, 'pointer-weight-reorder-proof-and-cleanup');
}

/** Native state/measure/timer transports preserve actual source, selection, and history ownership. */
function pointerFixture() {
  let state = weightStateFor('(cat:1.05), dog').update({
    effects: StateEffect.appendConfig.of([reorderPreview, reorderGrabCursor]),
  }).state;
  state = state.update({
    effects: installProjection.of(
      prepareProjection(state, state.field(semanticStructures).native!),
    ),
  }).state;
  const original = {
    document: globalThis.document,
    window: globalThis.window,
    Element: globalThis.Element,
    PointerEvent: globalThis.PointerEvent,
    getComputedStyle: globalThis.getComputedStyle,
    queueMicrotask: globalThis.queueMicrotask,
  };
  const roots: PortalElement[] = [],
    jobs: Measure[] = [],
    tasks: (() => void)[] = [];
  const timers = new Map<number, () => void>(),
    listeners = new Map<string, EventListenerOrEventListenerObject>();
  let timerId = 0;
  const label = new PortalElement();
  globalThis.document = {
    body: { append: (root: PortalElement) => roots.push(root) },
    documentElement: { clientWidth: 200, clientHeight: 200 },
    createElement: () => new PortalElement(),
    createElementNS: () => new PortalElement(),
  } as unknown as Document;
  globalThis.window = {
    setTimeout(callback: () => void) {
      timers.set(++timerId, callback);
      return timerId;
    },
    clearTimeout(id: number) {
      timers.delete(id);
    },
    addEventListener(name: string, callback: EventListenerOrEventListenerObject) {
      listeners.set(name, callback);
    },
    removeEventListener(name: string) {
      listeners.delete(name);
    },
  } as unknown as Window & typeof globalThis;
  globalThis.Element = PortalElement as unknown as typeof Element;
  globalThis.PointerEvent = PrimaryPointer as unknown as typeof PointerEvent;
  globalThis.getComputedStyle = () => ({ color: 'rgb(0, 0, 0)' }) as CSSStyleDeclaration;
  globalThis.queueMicrotask = (callback) => tasks.push(callback);
  const view = {
    get state() {
      return state;
    },
    compositionStarted: false,
    composing: false,
    scaleX: 1,
    scaleY: 1,
    contentDOM: { contains: (node: unknown) => node === label },
    scrollDOM: {
      getBoundingClientRect: () => ({
        left: 0,
        right: 200,
        top: 0,
        bottom: 200,
      }),
    },
    posAtDOM: () => 4,
    requestMeasure: (request: Measure) => jobs.push(request),
    plugin: () => null,
  } as unknown as EditorView;
  const owner = new observedModule.PointerWeights(view),
    root = roots[0]!;
  return {
    owner,
    view,
    root,
    jobs,
    timers,
    listeners,
    get state() {
      return state;
    },
    /** Number hover uses the production admission and measurement path. */
    hover() {
      owner.hover({
        target: label,
        clientX: 50,
        clientY: 50,
      } as unknown as MouseEvent);
    },
    /** Deliver actual prepared read/write phases, retaining jobs explicitly when required. */
    measure() {
      const job = jobs.shift()!;
      job.write?.(job.read(view), view);
    },
    /** The control view's genuine press callback owns this held repeat. */
    press() {
      root.children[0]!.listeners.get('pointerdown')!(new PrimaryPointer() as unknown as Event);
    },
    /** Accept one real native state transition and notify the actual view owner. */
    publish(spec: TransactionSpec) {
      const before = state;
      state = state.update(spec).state;
      owner.update({
        startState: before,
        state,
        geometryChanged: false,
        viewportChanged: false,
        selectionSet: false,
      } as ViewUpdate);
    },
    /** Release all actual owner resources before restoring the finite external boundary. */
    destroy() {
      try {
        owner.destroy();
        while (tasks.length) tasks.shift()!();
      } finally {
        Object.assign(globalThis, original);
      }
    },
  };
}

for (const mode of ['preview', 'grab'] as const) {
  /** Both genuine accepted reorder publications exclude the independently-owned numeric portal. */
  const enter = (f: ReturnType<typeof pointerFixture>): void => {
    if (mode === 'grab') f.publish({ effects: setReorderGrabCursor.of({}) });
    else {
      const capture = captureReorder(f.state);
      const preview = prepareReorderPreview(
        {
          token: {},
          capture,
          activeChip: capture.active,
          order: capture.prepared.initial,
        },
        'complete',
      );
      assert.ok(preview);
      f.publish({ effects: installReorderPreview.of(preview) });
      assert.equal(f.state.field(reorderPreview).value, preview);
    }
  };
  test(`${mode} blocks subsequent numeric hover and ordinary arrows return after cancellation`, () =>
    usingPointerOwner((f) => {
      const source = readExactSource(f.state),
        selection = f.state.selection;
      enter(f);
      f.hover();
      assert.equal(f.jobs.length, 0);
      assert.equal(f.root.hidden, true);
      f.publish({
        effects: [installReorderPreview.of(null), setReorderGrabCursor.of(null)],
      });
      f.hover();
      assert.equal(f.jobs.length, 1);
      f.measure();
      assert.equal(f.root.hidden, false);
      assert.equal(readExactSource(f.state), source);
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state), 0);
    }));
  test(`${mode} retires visible held arrows, pending measure writes, and repeat callbacks`, () =>
    usingPointerOwner((f) => {
      const source = readExactSource(f.state),
        selection = f.state.selection;
      f.hover();
      f.measure();
      assert.equal(f.root.hidden, false);
      f.press();
      assert.equal(f.timers.size, 1);
      const repeat = [...f.timers.values()][0]!;
      // An actual layout update queues the production remeasurement before mode admission.
      f.owner.update({
        startState: f.state,
        state: f.state,
        geometryChanged: true,
        viewportChanged: false,
        selectionSet: false,
      } as ViewUpdate);
      const pending = f.jobs.shift()!,
        result = pending.read(f.view);
      assert.ok(result);
      enter(f);
      assert.equal(f.root.hidden, true);
      assert.equal(f.timers.size + f.listeners.size, 0);
      pending.write?.(result, f.view);
      repeat();
      f.hover();
      assert.equal(f.root.hidden, true);
      assert.equal(f.jobs.length + f.timers.size + f.listeners.size, 0);
      assert.equal(readExactSource(f.state), source);
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state), 0);
    }));
}
