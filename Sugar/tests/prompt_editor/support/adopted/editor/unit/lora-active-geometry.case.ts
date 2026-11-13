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
/** Native hit-phase transport proves stationary hover retention and exact lifetime refusal; browser feedback owns physical layout. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect, type TransactionSpec } from '@codemirror/state';
import type { EditorView, ViewUpdate } from '@codemirror/view';
import { weightStateFor } from '../transaction/weight-command-state.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { prepareNativeStructure } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  installProjection,
  richProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  LoraActiveView,
  loraIntent,
  setLoraFocus,
  setLoraApplicationFocus,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-active.ts';

/** Capture the supported public request shape without importing CM's private declaration. */
type NativeMeasure = NonNullable<Parameters<EditorView['requestMeasure']>[0]>;

/** Exact literal element identities provide no test-side hit algorithm or geometry. */
class NativeHit {
  readonly position: number;
  /** Capture an independently supplied native logical hit, never a computed glyph boundary. */
  constructor(position: number) {
    this.position = position;
  }
  /** Production selects only the actual chip role. */
  closest(selector: string): this {
    assert.equal(selector, '.sp-lora-chip');
    return this;
  }
}

/** The actual native owner runs with independently delivered READ/WRITE phases and real immutable state. */
function acquire() {
  const source = '<lora:first:1.00>, <lora:second:0.50>, tail';
  let state = weightStateFor(source, source.length);
  state = state.update({
    effects: StateEffect.appendConfig.of(loraIntent),
  }).state;
  state = state.update({
    effects: [
      installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
      setLoraFocus.of(true),
      setLoraApplicationFocus.of(true),
    ],
  }).state;
  const first = new NativeHit(0),
    second = new NativeHit(19);
  let hit: NativeHit | null = first,
    phase = 'idle',
    retired = false,
    reads = 0;
  let duringHit: (() => void) | null = null;
  const requests: NativeMeasure[] = [];
  let owner: LoraActiveView | null = null;
  const document = {
    defaultView: new EventTarget(),
    hasFocus: () => true,
    /** A geometric update must never enter the native hit API before READ. */
    elementFromPoint(x: number, y: number) {
      assert.equal(retired, false);
      assert.equal(phase, 'read');
      assert.deepEqual([x, y], [31, 37]);
      reads++;
      const callback = duringHit;
      duringHit = null;
      callback?.();
      return hit;
    },
  };
  const view = {
    get state() {
      assert.equal(retired, false);
      return state;
    },
    hasFocus: true,
    dom: { ownerDocument: document },
    contentDOM: {
      contains: (node: unknown) => node === first || node === second,
    },
    posAtDOM(node: NativeHit, offset: number) {
      assert.equal(offset, 0);
      return node.position;
    },
    dispatch(spec: TransactionSpec) {
      const startState = state,
        transaction = state.update(spec);
      state = transaction.state;
      owner?.update({
        startState,
        state,
        transactions: [transaction],
        geometryChanged: false,
        viewportChanged: false,
      } as unknown as ViewUpdate);
    },
    /** Coalesce only identical actual measure keys, as the supported native view does. */
    requestMeasure(request: NativeMeasure) {
      const old = requests.findIndex((value) => value.key === request.key);
      if (old >= 0) requests.splice(old, 1);
      requests.push(request);
    },
  } as unknown as EditorView;
  owner = new LoraActiveView(view);
  return {
    owner,
    view,
    first,
    second,
    source,
    get state() {
      return state;
    },
    get reads() {
      return reads;
    },
    get queued() {
      return requests.length;
    },
    /** A real movement supplies only one finite current viewport point. */
    hover() {
      owner!.hover({
        target: first,
        clientX: 31,
        clientY: 37,
      } as unknown as MouseEvent);
    },
    /** The caller controls external hit outcomes, not the production admission decision. */
    hit(value: NativeHit | null) {
      hit = value;
    },
    duringRead(callback: () => void) {
      duringHit = callback;
    },
    geometry(viewport = false) {
      owner!.update({
        state,
        geometryChanged: !viewport,
        viewportChanged: viewport,
      } as ViewUpdate);
    },
    /** Execute the exact submitted callback in READ, retaining WRITE for independent cancellation controls. */
    read() {
      const request = requests.shift();
      assert.ok(request);
      phase = 'read';
      let value: unknown;
      try {
        value = request.read(view);
      } finally {
        phase = 'idle';
      }
      return () => {
        phase = 'write';
        try {
          request.write?.(value, view);
        } finally {
          phase = 'idle';
        }
      };
    },
    destroy() {
      owner!.destroy();
      retired = true;
    },
  };
}

/** Temporary Element identity and every admitted listener retire independently with raw primary precedence. */
async function fixture(body: (value: ReturnType<typeof acquire>) => Promise<void>): Promise<void> {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'Element'),
    causes: unknown[] = [];
  let value: ReturnType<typeof acquire> | null = null;
  try {
    Object.defineProperty(globalThis, 'Element', {
      configurable: true,
      value: NativeHit,
    });
    value = acquire();
    await body(value);
  } catch (cause) {
    causes.push(cause);
  }
  for (const retire of [
    () => value?.destroy(),
    () => {
      if (descriptor) Object.defineProperty(globalThis, 'Element', descriptor);
      else Reflect.deleteProperty(globalThis, 'Element');
    },
  ])
    try {
      retire();
    } catch (cause) {
      causes.push(cause);
    }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'LORA_HOVER_GEOMETRY_PROOF_RETIREMENT');
}

test('incidental geometry redraw retains exact stationary hover and canonical widget output only after actual READ', async () => {
  await fixture(async (f) => {
    f.hover();
    await Promise.resolve();
    const hover = f.state.field(loraIntent).hover,
      projection = f.state.field(richProjection);
    f.geometry();
    assert.equal(f.reads, 0);
    assert.equal(f.state.field(loraIntent).hover, hover);
    const write = f.read();
    assert.equal(f.reads, 1);
    write();
    await Promise.resolve();
    assert.equal(f.state.field(loraIntent).hover, hover);
    assert.equal(f.state.field(richProjection), projection);
    assert.equal(f.state.doc.toString(), f.source);
  });
});

for (const target of ['departed', 'different-chip'] as const)
  test(`actual viewport relocation ${target} clears old intent without adopting another token`, async () => {
    await fixture(async (f) => {
      f.hover();
      await Promise.resolve();
      f.hit(target === 'departed' ? null : f.second);
      f.geometry(true);
      const write = f.read();
      assert.ok(f.state.field(loraIntent).hover);
      write();
      await Promise.resolve();
      assert.equal(f.state.field(loraIntent).hover, null);
    });
  });

test('equivalent source-neutral state replacement reschedules pending READ instead of dropping its hover validation', async () => {
  await fixture(async (f) => {
    f.hover();
    await Promise.resolve();
    f.geometry();
    f.view.dispatch({ effects: setLoraFocus.of(true) });
    assert.equal(f.queued, 1);
    f.hit(null);
    f.read()();
    await Promise.resolve();
    assert.equal(f.state.field(loraIntent).hover, null);
  });
});

test('last native hit reentry and post-READ retirement cannot publish stale hover or access a retired view', async () => {
  await fixture(async (f) => {
    f.hover();
    await Promise.resolve();
    f.geometry();
    f.duringRead(() => f.view.dispatch({ effects: invalidateCommandContext.of(null) }));
    f.read()();
    await Promise.resolve();
    assert.equal(f.state.field(loraIntent).hover, null);
    f.hover();
    await Promise.resolve();
    f.geometry();
    const write = f.read();
    f.destroy();
    write();
    await Promise.resolve();
    assert.equal(f.reads, 2);
  });
});
