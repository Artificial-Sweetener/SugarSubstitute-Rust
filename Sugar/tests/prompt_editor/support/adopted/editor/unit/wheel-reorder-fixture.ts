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
/** Observe the actual wheel owner through public plugin registration and literal native platform boundaries. */
import assert from 'node:assert/strict';
import * as stateModule from '@codemirror/state';
import { ViewPlugin, EditorView, type ViewUpdate } from '@codemirror/view';
import { weightPreview } from '../../../../../../frontend/sugarprompt/editor/src/weight-preview.ts';
import '../../../../../../frontend/sugarprompt/editor/src/weight-interactions.ts';
import * as feedback from '../../../../../../frontend/sugarprompt/editor/src/wheel-feedback.ts';
import * as reorder from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import * as grab from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-cursor.ts';
import { weightStateFor } from '../transaction/weight-command-state.ts';
import { NativeReorderEntry } from '../../../../../../frontend/sugarprompt/editor/src/reorder-entry.ts';
import type { NativeReorderCapture } from '../../../../../../frontend/sugarprompt/editor/src/reorder-command.ts';
import type { CommandInputSettlement } from '../../../../../../frontend/sugarprompt/editor/src/input-settlement.ts';
import { installProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { captureReorder } from '../transaction/reorder-fixture.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';

/** Only the existing public plugin-value callbacks cross the supported registration boundary. */
interface WheelOwner {
  move(event: MouseEvent, activate?: boolean): void;
  wheel(event: WheelEvent): boolean;
  geometryChanged(): void;
  update(update: ViewUpdate): void;
  clear(): void;
  destroy(): void;
}
/** Validate the exact registered external value before adapting its existing callback contract. */
function wheelOwner(value: unknown): WheelOwner {
  if (!value || typeof value !== 'object') throw new TypeError('WHEEL_OWNER_OBJECT');
  for (const name of ['move', 'wheel', 'geometryChanged', 'update', 'clear', 'destroy'])
    if (typeof Reflect.get(value, name) !== 'function') throw new TypeError('WHEEL_OWNER_CALLBACK');
  return value as WheelOwner;
}
/** A finite fresh module identity prevents reuse of a previous registration in this process. */
let nextImport = 0;

/** Capture only the public registration of a fresh actual owner module, restoring the exact static method before use. */
async function registeredConstructor(path: string): Promise<unknown> {
  const descriptor = Object.getOwnPropertyDescriptor(ViewPlugin, 'fromClass');
  assert.ok(descriptor);
  const register = ViewPlugin.fromClass;
  let constructor: unknown,
    failed = false,
    primary: unknown;
  try {
    ViewPlugin.fromClass = ((value: unknown, ...args: unknown[]) => {
      constructor = value;
      return Reflect.apply(register, ViewPlugin, [value, ...args]);
    }) as typeof ViewPlugin.fromClass;
    await import(new URL(`${path}?wheel-owner=${++nextImport}`, import.meta.url).href);
  } catch (cause) {
    failed = true;
    primary = cause;
  }
  const failures: unknown[] = failed ? [primary] : [];
  try {
    Object.defineProperty(ViewPlugin, 'fromClass', descriptor);
  } catch (cause) {
    failures.push(cause);
  }
  if (failures.length === 1) throw failures[0];
  if (failures.length) throw new AggregateError(failures, 'wheel-fixture-registration-failed');
  if (typeof constructor !== 'function') throw new TypeError('WHEEL_OWNER_CONSTRUCTOR');
  return constructor;
}
/** The actual existing source queue is observed, never reimplemented by the native platform transport. */
interface WeightQueue {
  update(update: ViewUpdate): void;
  destroy(): void;
}
/** Real state/geometry/feedback policies run against literal external points, ranges, clocks and plugin registration. */
export async function wheelReorderFixture(
  kind: 'emphasis' | 'lora' | 'wildcard' | 'two-emphasis' = 'emphasis',
) {
  const source =
    kind === 'two-emphasis'
      ? '(cat:1.20), (dog:1.30)'
      : kind === 'emphasis'
        ? '(soft:2.00), next'
        : kind === 'lora'
          ? '<lora:real:0.70>, next'
          : '{one|2}, next';
  let state = weightStateFor(source, kind === 'emphasis' ? 2 : 0).update({
    effects: stateModule.StateEffect.appendConfig.of([
      reorder.reorderPreviewExtension,
      grab.reorderGrabCursor,
      feedback.wheelFeedback,
    ]),
  }).state;
  state = state.update({
    effects: installProjection.of(
      prepareProjection(state, state.field(semanticStructures).native!),
    ),
  }).state;
  let now = 0,
    nextTimer = 0,
    positionReads = 0,
    rangeReads = 0,
    geometryForbidden = false,
    commandLookups = 0;
  const jobs: NonNullable<Parameters<EditorView['requestMeasure']>[0]>[] = [],
    tasks: (() => void)[] = [],
    timers = new Map<number, { callback: () => void; due: number }>(),
    listeners = new Set<unknown>(),
    effects: unknown[] = [];
  const label = {
    dataset: {},
    matches(selector: string) {
      return kind === 'lora'
        ? selector === '.sp-lora-weight'
        : kind === 'wildcard' && selector.includes('sp-wildcard-tag');
    },
    getBoundingClientRect: () => ({
      left: 10,
      top: 10,
      width: 100,
      height: 20,
    }),
  };
  const chip = {
    querySelector: () => label,
    getBoundingClientRect: label.getBoundingClientRect,
  };
  const hit = {
      closest: (selector: string) =>
        selector === '.sp-lora-chip'
          ? kind === 'lora'
            ? chip
            : null
          : kind === 'wildcard'
            ? label
            : null,
    },
    nativeNode = {};
  const document = {
    elementFromPoint() {
      if (geometryForbidden) throw new Error('WHEEL_UNRELATED_HIT');
      return hit;
    },
    createRange() {
      if (geometryForbidden) throw new Error('WHEEL_UNRELATED_RANGE');
      rangeReads++;
      let start = 0;
      return {
        setStart(_node: unknown, offset: number) {
          start = offset;
        },
        setEnd() {},
        getClientRects: () => [
          {
            left: kind === 'two-emphasis' && start >= 12 ? 140 : 10,
            top: 10,
            width: 100,
            height: 20,
          },
        ],
      };
    },
  };
  const names = ['document', 'window', 'performance', 'queueMicrotask'] as const;
  const originals = new Map(
    names.map((name) => [name, Object.getOwnPropertyDescriptor(globalThis, name)]),
  );
  const attempted: (typeof names)[number][] = [];
  /** Restore each attempted platform binding independently, preserving raw cleanup causes. */
  function restore(): void {
    const failures: unknown[] = [];
    for (const name of attempted.splice(0).reverse()) {
      try {
        const original = originals.get(name);
        if (original) Object.defineProperty(globalThis, name, original);
        else if (!Reflect.deleteProperty(globalThis, name))
          throw new Error('WHEEL_FIXTURE_GLOBAL_RETIREMENT');
      } catch (cause) {
        failures.push(cause);
      }
    }
    if (failures.length === 1) throw failures[0];
    if (failures.length)
      throw new AggregateError(failures, 'wheel-fixture-platform-retirement-failed');
  }
  let owner: WheelOwner | null = null,
    commands: WeightQueue | null = null;
  const view = {
    get state() {
      return state;
    },
    composing: false,
    compositionStarted: false,
    contentDOM: { contains: () => true },
    viewport: { from: 0, to: state.doc.length },
    posAtCoords(point: { readonly x: number }) {
      if (geometryForbidden) throw new Error('WHEEL_UNRELATED_POSITION');
      positionReads++;
      return kind === 'two-emphasis' ? (point.x >= 140 ? 14 : 2) : kind === 'emphasis' ? 2 : 0;
    },
    posAtDOM: () => (kind === 'wildcard' ? 4 : 0),
    domAtPos: (position: number) => ({ node: nativeNode, offset: position }),
    focus() {},
    requestMeasure(request: NonNullable<Parameters<EditorView['requestMeasure']>[0]>) {
      jobs.push(request);
    },
    plugin(plugin: unknown) {
      if (plugin === weightPreview) return null;
      commandLookups++;
      return commands;
    },
    scrollSnapshot: () => EditorView.scrollIntoView(0),
    dispatch(spec: stateModule.TransactionSpec | readonly stateModule.Transaction[]) {
      const startState = state;
      if (Array.isArray(spec)) {
        for (const transaction of spec) {
          assert.equal(transaction.startState, state);
          state = transaction.state;
          effects.push(...transaction.effects);
        }
      } else {
        const transaction = state.update(spec as stateModule.TransactionSpec);
        state = transaction.state;
        effects.push(...transaction.effects);
      }
      const update = {
        state,
        startState,
        geometryChanged: false,
        viewportChanged: false,
      } as ViewUpdate;
      commands?.update(update);
      owner?.update(update);
    },
  } as unknown as EditorView;
  const window = {
    addEventListener(_name: string, listener: unknown) {
      listeners.add(listener);
    },
    removeEventListener(_name: string, listener: unknown) {
      listeners.delete(listener);
    },
    setTimeout(callback: () => void, delay = 0) {
      const id = ++nextTimer;
      timers.set(id, { callback, due: now + delay });
      return id;
    },
    clearTimeout(id: number) {
      timers.delete(id);
    },
  };
  const platform = {
    document,
    window,
    performance: { now: () => now },
    queueMicrotask(callback: () => void) {
      tasks.push(callback);
    },
  };
  try {
    for (const name of names) {
      attempted.push(name);
      Object.defineProperty(globalThis, name, {
        configurable: true,
        value: platform[name],
      });
    }
    const Queue = await registeredConstructor(
      '../../../../../../frontend/sugarprompt/editor/src/weight-interactions.ts',
    );
    if (typeof Queue !== 'function') throw new TypeError('WHEEL_QUEUE_CONSTRUCTOR');
    const value: unknown = Reflect.construct(Queue, [view]);
    if (
      !value ||
      typeof value !== 'object' ||
      typeof Reflect.get(value, 'update') !== 'function' ||
      typeof Reflect.get(value, 'destroy') !== 'function'
    )
      throw new TypeError('WHEEL_QUEUE_CALLBACK');
    commands = value as WeightQueue;
    const Wheel = await registeredConstructor(
      '../../../../../../frontend/sugarprompt/editor/src/weight-wheel.ts',
    );
    if (typeof Wheel !== 'function') throw new TypeError('WHEEL_OWNER_CONSTRUCTOR');
    owner = wheelOwner(Reflect.construct(Wheel, [view]));
  } catch (primary) {
    const failures: unknown[] = [primary];
    for (const release of [() => commands?.destroy(), () => restore()])
      try {
        release();
      } catch (cause) {
        failures.push(cause);
      }
    if (failures.length === 1) throw primary;
    throw new AggregateError(failures, 'wheel-fixture-acquisition-failed');
  }
  assert.ok(owner);
  /** Complete actual finite callback delivery without adding time or modeling native geometry. */
  function drain(): void {
    for (let step = 0; tasks.length && step < 32; step++) tasks.shift()!();
    assert.equal(tasks.length, 0);
  }

  /** Deliver only callbacks whose actual requested deadlines fit the controlled interval; never make a new hover dwell early. */
  function advance(milliseconds: number): void {
    now += milliseconds;
    const due = [...timers]
      .filter(([, timer]) => timer.due <= now)
      .sort((a, b) => a[1].due - b[1].due);
    for (const [id, timer] of due) if (timers.delete(id)) timer.callback();
  }
  return {
    owner,
    view,
    jobs,
    tasks,
    timers,
    effects,
    get state() {
      return state;
    },
    get positionReads() {
      return positionReads;
    },
    get rangeReads() {
      return rangeReads;
    },
    get commandLookups() {
      return commandLookups;
    },
    /** Poison every unrelated geometry boundary before suppressed work is delivered. */
    forbidGeometry() {
      geometryForbidden = true;
    },
    /** Re-enable only the controlled native geometry transport for genuine ordinary-mode restoration. */
    allowGeometry() {
      geometryForbidden = false;
    },
    /** Deliver actual read/write callbacks with optional separation of the existing native phases. */
    measure() {
      const job = jobs.shift();
      assert.ok(job);
      job.write?.(job.read(view), view);
    },
    /** Complete the finite source-neutral publication queue and refuse unexpected unbounded work. */
    drain,
    /** Actual clock advancement delivers the existing single readiness callback without sleeping. */
    dwell() {
      advance(400);
    },
    advance,
    /** Authoritative state changes use real effects; notification may be held to independently exercise deferred callback currentness. */
    publish(kind: 'preview' | 'grab' | 'ordinary', notify = true) {
      const startState = state;
      if (kind === 'preview') {
        // Genuine accepted entry captures after readiness retirement; leave queued source callbacks pending to test takeover currentness.
        if (state.field(feedback.wheelFeedback).target)
          state = state.update({
            effects: feedback.showWheelFeedback.of(null),
          }).state;
        const capture = captureReorder(state),
          value = prepareReorderPreview(
            {
              token: {},
              capture,
              activeChip: 0,
              order: capture.prepared.initial,
            },
            'complete',
          );
        assert.ok(value);
        state = state.update({
          effects: reorder.installReorderPreview.of(value),
        }).state;
      } else
        state = state.update({
          effects:
            kind === 'grab'
              ? grab.setReorderGrabCursor.of({})
              : [grab.setReorderGrabCursor.of(null), reorder.installReorderPreview.of(null)],
        }).state;
      if (notify)
        owner!.update({
          state,
          startState,
          geometryChanged: false,
          viewportChanged: false,
        } as ViewUpdate);
    },
    /** Genuine entry settles after the existing keyboard observer retires feedback, so capture uses the current canonical projection. */
    takeover(): NativeReorderCapture {
      const pending: ((position: number | null) => void)[] = [],
        captures: NativeReorderCapture[] = [],
        reservations: CommandInputSettlement[] = [];
      const entry = new NativeReorderEntry(view, {
        measure(_state, _pointer, complete) {
          pending.push(complete);
        },
        retireMeasurement() {
          pending.splice(0);
        },
        available: () => true,
        complete(outcome) {
          if (outcome.status === 'accepted') {
            captures.push(outcome.capture);
            reservations.push(outcome.reservation);
          }
        },
        failed() {},
      });
      const failures: unknown[] = [];
      try {
        owner!.clear();
        assert.equal(entry.begin(), true);
        drain();
        const settle = pending.shift();
        assert.ok(settle);
        settle(kind === 'emphasis' ? 2 : 0);
        const capture = captures[0];
        assert.ok(capture);
        const value = prepareReorderPreview(
          {
            token: {},
            capture,
            activeChip: 0,
            order: capture.prepared.initial,
          },
          'complete',
        );
        assert.ok(value);
        view.dispatch({ effects: reorder.installReorderPreview.of(value) });
        drain();
      } catch (cause) {
        failures.push(cause);
      }
      for (const release of [
        () => {
          for (const reservation of reservations) entry.release(reservation);
        },
        () => entry.destroy(),
      ])
        try {
          release();
        } catch (cause) {
          failures.push(cause);
        }
      if (failures.length === 1) throw failures[0];
      if (failures.length) throw new AggregateError(failures, 'wheel-fixture-entry-failed');
      const captured = captures[0];
      assert.ok(captured);
      return captured;
    },
    /** Restore the test-owned platform binding after exact plugin disposal, preserving its original failure. */
    destroy() {
      const failures: unknown[] = [];
      for (const release of [
        () => owner!.destroy(),
        () => commands!.destroy(),
        () => restore(),
        () => assert.equal(timers.size + listeners.size, 0),
      ])
        try {
          release();
        } catch (cause) {
          failures.push(cause);
        }
      if (failures.length === 1) throw failures[0];
      if (failures.length) throw new AggregateError(failures, 'wheel-fixture-retirement-failed');
    },
  };
}
