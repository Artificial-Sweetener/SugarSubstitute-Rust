import { editorPresentations } from '../../../EditorPresentationFixture.js';
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
/** Actual controller mount/retirement preserves ordered causes and attempts every acquired collaborator once. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { registerHooks } from 'node:module';

/** Literal collaborator order is the controller resource contract, independent of the observed call trace. */
const retirementOrder = ['host', 'source', 'search', 'surface', 'parent'] as const;
/** One finite collaborator identity belongs to this isolated native-construction transport. */
type Stage = (typeof retirementOrder)[number];
/** Mount failures retain raw values without using truthiness as error presence. */
interface MountFailure {
  readonly stage: 'surface' | 'search' | 'source' | 'publish';
  readonly cause: unknown;
}
/** The exact authored loader boundary supplies fault controls; it does not replace controller cleanup behavior. */
interface Control {
  readonly calls: Stage[];
  readonly aliveDuring: boolean[];
  readonly failures: Map<Stage, unknown>;
  mountFailure: MountFailure | null;
  reenter: (() => void) | null;
}
const surfaceUrl = new URL(
  '../../../../../../frontend/sugarprompt/editor/src/native-surface.ts',
  import.meta.url,
).href;
const sources = new Map([
  [
    surfaceUrl,
    `
    /** One literal external construction transport exposes no native DOM or rendering behavior. */
    export const control = { calls: [], aliveDuring: [], failures: new Map(), mountFailure: null, reenter: null, alive: () => true };
    /** Preserve independently supplied raw construction failure identity. */
    export function construct(stage) { if (control.mountFailure?.stage === stage) throw control.mountFailure.cause; }
    /** Observe actual controller ordering and revocation before independently failing one release. */
    export function release(stage) {
      control.calls.push(stage); control.aliveDuring.push(control.alive());
      control.reenter?.();
      if (control.failures.has(stage)) throw control.failures.get(stage);
    }
    /** Only native construction is adapted; actual immutable state/snapshot/controller bodies execute. */
    export class NativeSurface {
      /** Return the actual prepared state through an inert external view boundary. */
      constructor(_parent, state) { construct("surface"); this.view = { state, hasFocus: false, composing: false, compositionStarted: false, plugin: () => null }; }
      /** Delegate only the external disposal call to the controlled transport. */
      dispose() { release("surface"); }
    }
  `,
  ],
  [
    new URL('../../../../../../frontend/sugarprompt/editor/src/tooltip-parent.ts', import.meta.url)
      .href,
    `
    import { release } from ${JSON.stringify(surfaceUrl)};
    /** Native popup construction is inert; the controller still owns disposal ordering. */
    export class NativeTooltipParent {
      configuration = [];
      /** Observe the parent release even after another collaborator rejects. */
      dispose() { release("parent"); }
    }
    /** Inert construction cannot certify native placement. */
    export function nativeTooltipSpace() { throw new Error("INERT_TOOLTIP_GEOMETRY_UNEXPECTED"); }
  `,
  ],
  [
    new URL(
      '../../../../../../frontend/sugarprompt/editor/src/native-host-updates.ts',
      import.meta.url,
    ).href,
    `
    import { control, release } from ${JSON.stringify(surfaceUrl)};
    /** Keep only the external host-update lifetime; cleanup remains in the actual controller. */
    export class NativeHostUpdates {
      /** Observe the actual controller's current liveness during every release. */
      constructor(owner) { control.alive = owner.alive; }
      /** Initial publication remains admitted in this inert host transport. */
      get publishing() { return false; }
      /** This cleanup proof never invokes host mutation. */
      apply() { throw new Error("INERT_HOST_MUTATION_UNEXPECTED"); }
      /** Dispose is an independently controlled external failure boundary. */
      dispose() { release("host"); }
    }
  `,
  ],
  [
    new URL('../../../../../../frontend/sugarprompt/editor/src/native-search.ts', import.meta.url)
      .href,
    `
    import { construct, release } from ${JSON.stringify(surfaceUrl)};
    /** Search acquisition/release are external collaborators, not copied cleanup policy. */
    export class NativeSearch {
      /** Fail only the requested construction frontier. */
      constructor() { construct("search"); }
      /** Snapshot publication needs no simulated search behavior. */
      observe() {}
      /** Independently reject release when requested. */
      dispose() { release("search"); }
    }
  `,
  ],
  [
    new URL(
      '../../../../../../frontend/sugarprompt/editor/src/native-source-commands.ts',
      import.meta.url,
    ).href,
    `
    import { construct, release } from ${JSON.stringify(surfaceUrl)};
    /** This owner proof never executes source commands; it observes their acquired lifetime. */
    export function nativeSourceCommands() { construct("source"); return { api: {}, dispose() { release("source"); } }; }
  `,
  ],
]);
const hook = registerHooks({
  /** Adapt exactly the five collaborator seams; no disposal decision is implemented in the loader. */
  load(url, context, next) {
    const source = sources.get(url);
    return source === undefined
      ? next(url, context)
      : { format: 'module', shortCircuit: true, source };
  },
});
test.after(() => hook.deregister());
const { mountEditor } = await import(
  '../../../../../../frontend/sugarprompt/editor/src/controller.ts'
);
// This exact authored JavaScript loader export is the only untyped transport boundary.
const { control } = (await import(
  '../../../../../../frontend/sugarprompt/editor/src/native-surface.ts'
)) as unknown as {
  readonly control: Control;
};

/** Reset only controlled external outcomes before one actual controller mount. */
function reset(): void {
  control.calls.length = 0;
  control.aliveDuring.length = 0;
  control.failures.clear();
  control.mountFailure = null;
  control.reenter = null;
}
/** Keep normal native state/configuration/snapshot preparation and the actual facade lifecycle. */
function mount() {
  return mountEditor({} as HTMLElement, {
    ...editorPresentations,
    source: 'plain',
    label: 'Test',
    placeholder: '',
    readOnly: false,
    /** Initial consumer failure is an actual late acquisition frontier. */
    onUpdate() {
      if (control.mountFailure?.stage === 'publish') throw control.mountFailure.cause;
    },
  });
}
/** Catch by count to retain raw undefined as a genuine thrown value. */
function caught(action: () => void): unknown {
  const failures: unknown[] = [];
  try {
    action();
  } catch (cause) {
    failures.push(cause);
  }
  assert.equal(failures.length, 1);
  return failures[0];
}
/** Every attempted release must observe an already-revoked controller, including reentrant callbacks. */
function retired(expected: readonly Stage[]): void {
  assert.deepEqual(control.calls, expected);
  assert.deepEqual(
    control.aliveDuring,
    expected.map(() => false),
  );
}

test('surface constructor and parent disposal failures preserve raw primary then cleanup cause', () => {
  reset();
  control.mountFailure = { stage: 'surface', cause: null };
  control.failures.set('parent', undefined);
  const failure = caught(() => {
    mount();
  });
  assert.ok(failure instanceof AggregateError);
  assert.deepEqual(failure.errors, [null, undefined]);
  retired(['host', 'parent']);
});

test('surface construction retains raw undefined after all previously acquired owners retire', () => {
  reset();
  control.mountFailure = { stage: 'surface', cause: undefined };
  assert.equal(
    caught(() => {
      mount();
    }),
    undefined,
  );
  retired(['host', 'parent']);
});

for (const [stage, expected] of [
  ['search', ['host', 'surface', 'parent']],
  ['source', ['host', 'search', 'surface', 'parent']],
  ['publish', ['host', 'source', 'search', 'surface', 'parent']],
] as const)
  test(`late ${stage} acquisition failure retires every already-acquired collaborator`, () => {
    reset();
    const primary = Symbol(stage);
    control.mountFailure = { stage, cause: primary };
    assert.equal(
      caught(() => {
        mount();
      }),
      primary,
    );
    retired(expected);
  });

for (const stage of retirementOrder)
  test(`throwing ${stage} disposal cannot skip another owner or permit repeated disposal`, () => {
    reset();
    const api = mount(),
      primary = Symbol(stage);
    control.failures.set(stage, primary);
    assert.equal(
      caught(() => api.destroy()),
      primary,
    );
    retired(retirementOrder);
    api.destroy();
    retired(retirementOrder);
    assert.throws(() => api.snapshot(), { message: 'Editor disposed' });
  });

test('surface and parent teardown failures are both retained in ownership order', () => {
  reset();
  const api = mount();
  control.failures.set('surface', null);
  control.failures.set('parent', undefined);
  const failure = caught(() => api.destroy());
  assert.ok(failure instanceof AggregateError);
  assert.deepEqual(failure.errors, [null, undefined]);
  retired(retirementOrder);
  api.destroy();
  retired(retirementOrder);
});

test('all five independent teardown failures retain count, raw values and exact order', () => {
  reset();
  const api = mount(),
    search = Symbol('search'),
    surface = new Error('NATIVE_RELEASE'),
    parent = Symbol('parent');
  control.failures.set('host', null);
  control.failures.set('source', undefined);
  control.failures.set('search', search);
  control.failures.set('surface', surface);
  control.failures.set('parent', parent);
  const failure = caught(() => api.destroy());
  assert.ok(failure instanceof AggregateError);
  assert.deepEqual(failure.errors, [null, undefined, search, surface, parent]);
  assert.equal(failure.errors[3], surface);
  retired(retirementOrder);
  api.destroy();
  retired(retirementOrder);
});

test('reentrant destruction sees revoked authority and releases each owner once', () => {
  reset();
  const api = mount();
  control.reenter = () => api.destroy();
  api.destroy();
  retired(retirementOrder);
  api.destroy();
  retired(retirementOrder);
});

for (const primary of [null, undefined])
  test(`single raw ${String(primary)} release failure is not replaced by an aggregate`, () => {
    reset();
    const api = mount();
    control.failures.set('surface', primary);
    assert.equal(
      caught(() => api.destroy()),
      primary,
    );
    retired(retirementOrder);
    api.destroy();
    retired(retirementOrder);
  });
