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
/** Actual tooltip factory/session owners reject stale measured/deferred output; native CSS and wheel geometry remain browser proof. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { showTooltip, type EditorView, type TooltipView, type ViewUpdate } from '@codemirror/view';
import type { TransactionSpec } from '@codemirror/state';
import { completionTooltips } from '../../../../../../frontend/sugarprompt/editor/src/completion-tooltip.ts';
import {
  completionSession,
  installCompletion,
  selectCompletion,
} from '../../../../../../frontend/sugarprompt/editor/src/completion-session.ts';
import { completionWall } from '../../../../../../frontend/sugarprompt/editor/src/completion-wall-state.ts';
import { loraMedia } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-state.ts';
import { completionState, sessionResult } from '../transaction/completion-session-fixture.ts';
import { CompletionDocument, CompletionNode } from './completion-dom-fixture.ts';

/** One native request carries the actual owner callback pair without duplicating their behavior. */
interface Job {
  readonly key?: unknown;
  readonly read: (view: EditorView) => unknown;
  readonly write?: (value: unknown, view: EditorView) => void;
}

/** Acquire real completion state/factory and independently controlled DOM/measure/microtask transports. */
function fixture() {
  const doc = new CompletionDocument(),
    jobs: Job[] = [],
    microtasks: VoidFunction[] = [],
    resize: VoidFunction[] = [];
  let schedulingFailure: { cause: unknown } | null = null,
    disconnectFailure: { cause: unknown } | null = null,
    disconnected = 0;
  const originals = new Map(
    ['document', 'ResizeObserver', 'queueMicrotask'].map((key) => [
      key,
      Object.getOwnPropertyDescriptor(globalThis, key),
    ]),
  );
  /** Native observation only announces invalidation and owns no layout policy. */
  class Observer {
    /** Retain exactly the callback registered by the actual owner. */
    constructor(callback: VoidFunction) {
      resize.push(callback);
    }
    /** Observation has no automatic fabricated geometry. */
    observe(): void {}
    /** Disposal prevents further automatic callbacks; explicit stale jobs remain controlled by their owner. */
    disconnect(): void {
      disconnected++;
      if (disconnectFailure) throw disconnectFailure.cause;
    }
  }
  let tooltip: TooltipView | null = null;
  /** All globals and the actual tooltip resource retire independently after the control body. */
  function restore(): void {
    const causes: unknown[] = [];
    try {
      tooltip?.destroy?.();
    } catch (error) {
      causes.push(error);
    }
    for (const [key, descriptor] of originals) {
      try {
        if (descriptor) Object.defineProperty(globalThis, key, descriptor);
        else Reflect.deleteProperty(globalThis, key);
      } catch (error) {
        causes.push(error);
      }
    }
    if (causes.length === 1) throw causes[0];
    if (causes.length > 1) throw new AggregateError(causes, 'completion-fixture-cleanup');
  }
  try {
    Object.defineProperty(globalThis, 'document', {
      value: doc,
      configurable: true,
    });
    Object.defineProperty(globalThis, 'ResizeObserver', {
      value: Observer,
      configurable: true,
    });
    Object.defineProperty(globalThis, 'queueMicrotask', {
      value: (callback: VoidFunction) => microtasks.push(callback),
      configurable: true,
    });
    let state = completionState('<lora:ha', undefined, [completionTooltips, loraMedia]);
    const rows = Array.from({ length: 12 }, (_, index) => ({
      name: `ha${index}`,
      backend: `ha${index}.pt`,
    }));
    let result = sessionResult(state, rows, 'lora');
    state = state.update({ effects: installCompletion.of(result) }).state;
    const view = {
      get state() {
        return state;
      },
      coordsAtPos() {
        return { left: 90, right: 91, top: 110, bottom: 130 };
      },
      defaultLineHeight: 19,
      requestMeasure(job: Job) {
        if (schedulingFailure) throw schedulingFailure.cause;
        jobs.push(job);
      },
      dispatch(spec: TransactionSpec) {
        const startState = state;
        state = state.update(spec).state;
        tooltip?.update?.({ startState, state } as ViewUpdate);
      },
    } as unknown as EditorView;
    const factory = state.facet(showTooltip)[0];
    assert.ok(factory);
    tooltip = factory.create(view);
    const root = tooltip.dom as unknown as CompletionNode,
      viewport = root.children[0]!;
    viewport.clientWidth = 390;
    viewport.clientHeight = 100;
    viewport.scrollHeight = 720;
    /** Deliver READ while guarding every native geometry endpoint; WRITE remains geometry-free. */
    const read = (job: Job): unknown => {
      doc.readsAllowed = true;
      try {
        return job.read(view);
      } finally {
        doc.readsAllowed = false;
      }
    };
    /** Select only the public factory's actual tooltip request, leaving title requests independent. */
    const take = (): Job => {
      const index = jobs.findIndex((job) => job.key === tooltip);
      assert.ok(index >= 0);
      return jobs.splice(index, 1)[0]!;
    };
    /** Deliver one complete real tooltip phase without inventing an idle-frame delay. */
    const flush = (): void => {
      const job = take();
      job.write?.(read(job), view);
    };
    /** Flush only queued owner microtasks, retaining new requests for explicit subsequent native phases. */
    const deferred = (): void => {
      const batch = microtasks.splice(0);
      for (const callback of batch) callback();
    };
    /** Publish genuine newly prepared candidates through the same native session state field. */
    const replace = (): void => {
      result = sessionResult(state, rows, 'lora');
      view.dispatch({ effects: installCompletion.of(result) });
    };
    tooltip.mount?.(view);
    return {
      view,
      tooltip,
      viewport,
      jobs,
      microtasks,
      resize,
      read,
      take,
      flush,
      deferred,
      replace,
      restore,
      state: () => state,
      result: () => result,
      /** Inject only the actual later native queue/disconnect boundaries. */
      failScheduling: (primary: unknown, cleanup: unknown) => {
        schedulingFailure = { cause: primary };
        disconnectFailure = { cause: cleanup };
      },
      disconnected: () => disconnected,
    };
  } catch (error) {
    try {
      restore();
    } catch (cleanup) {
      throw new AggregateError([error, cleanup], 'completion-fixture-acquisition');
    }
    throw error;
  }
}

test('a replaced result between native READ and WRITE cannot publish the previous wall dimensions', () => {
  const f = fixture();
  try {
    const old = f.take(),
      measured = f.read(old);
    f.viewport.clientWidth = 558;
    f.replace();
    old.write?.(measured, f.view);
    f.deferred();
    assert.equal(f.state().field(completionWall) === null, true);
    f.flush();
    f.deferred();
    const wall = f.state().field(completionWall);
    assert.ok(wall);
    assert.equal(wall.result === f.result(), true);
    assert.equal(wall.layout.width, 558);
  } finally {
    f.restore();
  }
});

test('retired and reinstalled identical candidates cannot adopt an older deferred presentation', () => {
  const f = fixture();
  try {
    f.flush();
    assert.equal(f.microtasks.length, 1);
    const result = f.result(),
      previous = f.state().field(completionSession)!.presentation;
    f.view.dispatch({ effects: installCompletion.of(null) });
    f.view.dispatch({ effects: installCompletion.of(result) });
    assert.notEqual(f.state().field(completionSession)!.presentation, previous);
    f.deferred();
    assert.equal(f.state().field(completionWall) === null, true);
    f.flush();
    f.deferred();
    assert.equal(f.state().field(completionWall)?.result === result, true);
  } finally {
    f.restore();
  }
});

test('resize rejects deferred geometry and a repeated unchanged placement notification retains an admitted thumb capture', () => {
  const f = fixture();
  try {
    f.flush();
    f.viewport.clientWidth = 558;
    f.resize[0]!();
    f.deferred();
    assert.equal(f.state().field(completionWall) === null, true);
    f.flush();
    f.deferred();
    assert.equal(f.state().field(completionWall)?.layout.width, 558);
    f.tooltip.positioned?.({ left: 0, right: 800, top: 0, bottom: 800 });
    f.flush();
    f.flush();
    f.deferred();
    const thumb = f.viewport.children.find((node) => node.className === 'sp-completion-scrollbar')!
      .children[0]!;
    const down = new Event('pointerdown', { cancelable: true });
    Object.defineProperties(down, {
      button: { value: 0 },
      pointerId: { value: 7 },
      clientY: { value: 10 },
    });
    thumb.dispatchEvent(down);
    f.flush();
    f.deferred();
    assert.equal(thumb.captures.has(7), true);
    f.tooltip.positioned?.({ left: 0, right: 800, top: 0, bottom: 800 });
    f.flush();
    f.deferred();
    assert.equal(thumb.captures.has(7), true);
  } finally {
    f.restore();
  }
});

test('a newly unavailable completion viewport synchronously releases its actual thumb capture before observer delivery', () => {
  const f = fixture();
  try {
    f.tooltip.positioned?.({ left: 0, right: 800, top: 0, bottom: 800 });
    f.flush();
    f.flush();
    f.deferred();
    const thumb = f.viewport.children[0]!.children[0]!;
    const down = new Event('pointerdown', { cancelable: true });
    Object.defineProperties(down, {
      button: { value: 0 },
      pointerId: { value: 7 },
      clientY: { value: 10 },
    });
    thumb.dispatchEvent(down);
    f.flush();
    assert.equal(thumb.captures.has(7), true);
    const state = f.state();
    f.tooltip.positioned?.({ left: 0, right: 800, top: 0, bottom: 140 });
    f.flush();
    assert.equal(thumb.captures.has(7), false);
    assert.equal(thumb.released, 1);
    assert.equal(f.tooltip.dom.style.visibility, 'hidden');
    assert.equal(f.state() === state, true);
    f.tooltip.positioned?.({ left: 0, right: 800, top: 0, bottom: 800 });
    f.flush();
    f.flush();
    assert.equal(f.tooltip.dom.style.visibility, '');
    assert.equal(thumb.captures.has(7), false);
  } finally {
    f.restore();
  }
});

test("reentrant native scroll assignment cannot consume the newer selected row's ensure-visible intent", () => {
  const f = fixture();
  try {
    f.flush();
    f.deferred();
    let top = 0,
      once = true;
    Object.defineProperty(f.viewport, 'scrollTop', {
      configurable: true,
      get: () => top,
      set: (value: number) => {
        top = value;
        if (once) {
          once = false;
          f.view.dispatch({
            effects: selectCompletion.of({ result: f.result(), index: 10 }),
          });
        }
      },
    });
    f.view.dispatch({
      effects: selectCompletion.of({ result: f.result(), index: 1 }),
    });
    f.flush();
    f.deferred();
    assert.equal(f.state().field(completionSession)?.selected, 10);
    f.flush();
    f.deferred();
    assert.equal(top, 620);
  } finally {
    f.restore();
  }
});

test('later native observer scheduling failure retires all output and preserves raw primary before disconnect failure', () => {
  const f = fixture();
  try {
    f.flush();
    f.deferred();
    const wall = f.state().field(completionWall),
      carrier = f.viewport.children[0]!;
    f.failScheduling(null, undefined);
    assert.throws(
      () => f.resize[0]!(),
      (error: unknown) => {
        assert.ok(error instanceof AggregateError);
        assert.deepEqual(error.errors, [null, undefined]);
        return true;
      },
    );
    assert.equal(f.disconnected(), 1);
    assert.equal(carrier.parentElement, null);
    f.tooltip.positioned?.({ left: 0, right: 800, top: 0, bottom: 800 });
    f.deferred();
    assert.equal(f.state().field(completionWall) === wall, true);
  } finally {
    f.restore();
  }
});

test('selected-row publication capture failure retires the complete tooltip before raw rethrow', () => {
  const f = fixture();
  try {
    f.flush();
    f.deferred();
    const carrier = f.viewport.children[0]!,
      thumb = carrier.children[0]!,
      raw = Object.freeze({ code: 2 });
    const down = new Event('pointerdown', { cancelable: true });
    Object.defineProperties(down, {
      button: { value: 0 },
      pointerId: { value: 7 },
      clientY: { value: 10 },
    });
    thumb.dispatchEvent(down);
    f.flush();
    f.deferred();
    assert.equal(thumb.captures.has(7), true);
    thumb.releasePointerCapture = () => {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      throw raw;
    };
    assert.throws(
      () =>
        f.view.dispatch({
          effects: selectCompletion.of({ result: f.result(), index: 1 }),
        }),
      (error: unknown) => error === raw,
    );
    assert.equal(f.disconnected(), 1);
    assert.equal(carrier.parentElement, null);
    f.tooltip.positioned?.({ left: 0, right: 800, top: 0, bottom: 800 });
  } finally {
    f.restore();
  }
});
