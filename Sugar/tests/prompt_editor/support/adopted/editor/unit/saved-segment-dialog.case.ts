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
/** Actual modal owner retirement is tested with a literal DOM transport; browser focus remains native proof. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection, EditorState, type TransactionSpec } from '@codemirror/state';
import type { EditorView, ViewUpdate } from '@codemirror/view';
import { SavedSegmentActions } from '../../../../../../frontend/sugarprompt/editor/src/saved-segment-actions.ts';
import {
  savedSegmentHost,
  savedSegments,
  prepareSavedSegmentMenu,
  type SavedSegmentHost,
} from '../../../../../../frontend/sugarprompt/editor/src/saved-segment-menu.ts';
import { structureState } from '../transaction/document-structure-fixture.ts';
import type { FocusHandoff } from '../../../../../../frontend/sugarprompt/editor/src/focus-handoff-contracts.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

/** Only external DOM methods are substituted; real source, selection, publication and service owners remain executable. */
class LiteralNode {
  readonly children: LiteralNode[] = [];
  readonly listeners = new Map<string, EventListenerOrEventListenerObject>();
  parentNode: LiteralNode | null = null;
  isConnected = false;
  open = false;
  textContent = '';
  value = '';
  disabled = false;
  readonly failures: Partial<Record<'showModal' | 'focus' | 'close' | 'remove', Error>>;
  readonly tag: string;
  /** Each node shares the fixture's exact injected failure identities. */
  constructor(tag: string, failures: LiteralNode['failures']) {
    this.tag = tag;
    this.failures = failures;
  }
  /** Literal append is the only external mounting transport. */
  append(...nodes: LiteralNode[]): void {
    for (const node of nodes) {
      this.children.push(node);
      node.parentNode = this;
      node.isConnected = this.isConnected;
    }
  }
  /** The native fallback removes the same mounted dialog after a failed convenience removal. */
  removeChild(node: LiteralNode): void {
    this.children.splice(this.children.indexOf(node), 1);
    node.parentNode = null;
    node.isConnected = false;
  }
  /** Dynamic attributes remain literal. */
  setAttribute(): void {}
  /** Register only the actual owner callbacks. */
  addEventListener(name: string, listener: EventListenerOrEventListenerObject): void {
    this.listeners.set(name, listener);
  }
  /** Release callbacks so old modal actions cannot run after retirement. */
  removeEventListener(name: string): void {
    this.listeners.delete(name);
  }
  /** Opening can fail after native attachment. */
  showModal(): void {
    this.open = true;
    if (this.failures.showModal) throw this.failures.showModal;
  }
  /** Native input focus is an injected external boundary. */
  focus(): void {
    if (this.failures.focus) throw this.failures.focus;
  }
  /** Closing and removal are independent retirement obligations. */
  close(): void {
    this.open = false;
    if (this.failures.close) throw this.failures.close;
  }
  /** An external removal failure must not prevent fallback detachment. */
  remove(): void {
    if (this.failures.remove) throw this.failures.remove;
    this.parentNode?.removeChild(this);
  }
  /** Dispatch only a fixture intent into the real registered listener. */
  emit(name: string): void {
    const listener = this.listeners.get(name),
      event = { preventDefault() {} } as Event;
    if (typeof listener === 'function') listener(event);
    else listener?.handleEvent(event);
  }
}

/** Prepare a genuine source selection and service publication before mounting the modal owner. */
async function fixture() {
  const failures: LiteralNode['failures'] = {},
    nodes: LiteralNode[] = [],
    body = new LiteralNode('body', failures);
  body.isConnected = true;
  const document = {
    body,
    createElement(tag: string) {
      const node = new LiteralNode(tag, failures);
      nodes.push(node);
      return node;
    },
  };
  let signal: AbortSignal | undefined,
    saved = 0,
    restores = 0,
    cancels = 0;
  let complete: (() => void) | undefined;
  const viewportIntents: Parameters<FocusHandoff['restore']>[0][] = [];
  const host: SavedSegmentHost = {
    context: {
      mode: 'library',
      scopes: [{ kind: 'global', provider: null, key: 'global', label: 'Global' }],
    },
    adapter: {
      async read() {
        return { status: 'ready', segments: [] };
      },
      /** Hold the external write until explicit acknowledgement, independently of cancellation. */
      save(_value, next) {
        signal = next;
        saved++;
        return new Promise((resolve) => {
          complete = () => resolve({ status: 'saved' });
        });
      },
    },
    report(cause) {
      throw cause;
    },
  };
  const native = structureState('😀ab\r\ntail', 'prompt', [
    savedSegments,
    savedSegmentHost.of(host),
  ]);
  let state = native.state;
  state = state.update({ selection: EditorSelection.single(6, 2) }).state;
  const view = {
    get state() {
      return state;
    },
    dom: { ownerDocument: document },
    dispatch(spec: TransactionSpec) {
      state = state.update(spec).state;
    },
    composing: false,
    compositionStarted: false,
  } as unknown as EditorView;
  const owner = new SavedSegmentActions(view);
  // The actual owner schedules configuration, read and publication through bounded promise turns.
  for (let turn = 0; turn < 16 && state.field(savedSegments)?.status !== 'ready'; turn++)
    await Promise.resolve();
  assert.equal(state.field(savedSegments)?.status, 'ready');
  const handoff: FocusHandoff = {
    /** Observe the owner's viewport intent without controlling browser focus or selection. */
    async restore(viewport) {
      restores++;
      viewportIntents.push(viewport);
      return true;
    },
    cancel() {
      cancels++;
    },
  };
  return {
    owner,
    nodes,
    body,
    failures,
    handoff,
    permission: native.permission,
    menu: prepareSavedSegmentMenu(state, 0),
    /** Observe the actual immutable CodeMirror state retained by the transport. */
    state: () => state,
    viewportIntents,
    /** Settle the actual adapter operation independently of the owner acknowledgement. */
    complete() {
      const release = complete;
      complete = undefined;
      release?.();
    },
    /** Apply a real native transaction before notifying the actual plugin owner. */
    update(spec: TransactionSpec) {
      const startState = state;
      state = state.update(spec).state;
      owner.update({ startState, state, view } as ViewUpdate);
    },
    get signal() {
      return signal;
    },
    get saved() {
      return saved;
    },
    get cancels() {
      return cancels;
    },
    get restores() {
      return restores;
    },
  };
}

for (const completion of ['cancel', 'save'] as const)
  test(`${completion} restores source-neutral focus without requesting a viewport reveal`, async () => {
    const f = await fixture(),
      before = f.state();
    try {
      f.owner.save(f.menu, f.handoff);
      if (completion === 'cancel') f.nodes.find((node) => node.tag === 'dialog')!.emit('cancel');
      else {
        f.nodes.find((node) => node.tag === 'input')!.value = 'label';
        f.nodes.find((node) => node.tag === 'form')!.emit('submit');
        f.complete();
        for (let turn = 0; turn < 16 && f.restores === 0; turn++) await Promise.resolve();
      }
      assert.equal(f.restores, 1);
      assert.deepEqual(f.viewportIntents, ['preserve-viewport']);
      assert.equal(f.cancels, 0);
      assert.equal(readExactSource(f.state()), readExactSource(before));
      assert.equal(f.state().selection.eq(before.selection), true);
      assert.equal(f.body.children.length, 0);
      assert.equal(
        f.nodes.every((node) => node.listeners.size === 0),
        true,
      );
    } finally {
      f.owner.destroy();
      f.complete();
    }
  });

for (const fault of ['showModal', 'focus'] as const)
  test(`${fault} construction failure retires mounted DOM, listeners and focus with original cause`, async () => {
    const f = await fixture(),
      cause = new Error(`fixture-${fault}`);
    f.failures[fault] = cause;
    assert.throws(
      () => f.owner.save(f.menu, f.handoff),
      (error: unknown) => error === cause,
    );
    assert.equal(f.body.children.length, 0);
    assert.equal(
      f.nodes.every((node) => node.listeners.size === 0),
      true,
    );
    assert.equal(f.cancels, 1);
    assert.equal(f.restores, 0);
    f.owner.destroy();
  });

test('teardown attempts request abort, close, fallback removal, listeners and focus despite both DOM failures', async () => {
  const f = await fixture();
  f.owner.save(f.menu, f.handoff);
  f.nodes.find((node) => node.tag === 'input')!.value = 'label';
  f.nodes.find((node) => node.tag === 'form')!.emit('submit');
  assert.equal(f.saved, 1);
  const close = new Error('fixture-close'),
    remove = new Error('fixture-remove');
  f.failures.close = close;
  f.failures.remove = remove;
  assert.throws(
    () => f.owner.destroy(),
    (error: unknown) =>
      error instanceof AggregateError &&
      error.errors.length === 2 &&
      error.errors[0] === close &&
      error.errors[1] === remove,
  );
  assert.equal(f.signal?.aborted, true);
  assert.equal(f.body.children.length, 0);
  assert.equal(
    f.nodes.every((node) => node.listeners.size === 0),
    true,
  );
  assert.equal(f.cancels, 1);
  assert.equal(f.restores, 0);
  f.owner.destroy();
});

test('construction and cleanup failures preserve both causes after all retirement attempts', async () => {
  const f = await fixture(),
    primary = new Error('fixture-show'),
    cleanup = new Error('fixture-close');
  f.failures.showModal = primary;
  f.failures.close = cleanup;
  assert.throws(
    () => f.owner.save(f.menu, f.handoff),
    (error: unknown) =>
      error instanceof AggregateError && error.errors[0] === primary && error.errors[1] === cleanup,
  );
  assert.equal(f.body.children.length, 0);
  assert.equal(
    f.nodes.every((node) => node.listeners.size === 0),
    true,
  );
  assert.equal(f.cancels, 1);
  f.owner.destroy();
});

for (const change of ['source', 'selection', 'permission'] as const)
  test(`${change} retirement releases a held modal request without restoring old focus`, async () => {
    const f = await fixture();
    f.owner.save(f.menu, f.handoff);
    f.nodes.find((node) => node.tag === 'input')!.value = 'label';
    f.nodes.find((node) => node.tag === 'form')!.emit('submit');
    if (change === 'source') f.update({ changes: { from: 2, insert: 'x' } });
    else if (change === 'selection') f.update({ selection: EditorSelection.single(1) });
    else
      f.update({
        effects: f.permission.reconfigure(EditorState.readOnly.of(true)),
      });
    assert.equal(f.signal?.aborted, true);
    assert.equal(f.body.children.length, 0);
    assert.equal(
      f.nodes.every((node) => node.listeners.size === 0),
      true,
    );
    assert.equal(f.cancels, 1);
    assert.equal(f.restores, 0);
    f.owner.destroy();
  });
