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
/** Native permission cancellation and complete reentrant host adoption share one immutable state/history authority. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { registerHooks } from 'node:module';
import { readFileSync } from 'node:fs';
import { Transaction } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import { undo, redo, undoDepth, redoDepth } from '@codemirror/commands';

const observed = new URL(
  '../../../../../../frontend/sugarprompt/editor/src/composition-history.ts',
  import.meta.url,
).href;
const hook = registerHooks({
  /** Export-only observation leaves the actual cancellation and permission owner bodies unchanged. */
  load(url, context, next) {
    const result = next(url, context);
    return url === observed
      ? {
          ...result,
          source:
            readFileSync(new URL(url), 'utf8') +
            '\nexport { CompositionHistory, nativeCompositionHistory };\n',
        }
      : result;
  },
});
const { hostFixture } = await import('./host-document-fixture.ts');

test('newer exact edit during revocation settlement preserves the complete transaction and permission', async () => {
  const module = (await import(
    '../../../../../../frontend/sugarprompt/editor/src/composition-history.ts'
  )) as unknown as {
    CompositionHistory: new (view: EditorView) => {
      start(): void;
      update(value: { transactions: readonly Transaction[] }): void;
      destroy(): void;
    };
    nativeCompositionHistory: unknown;
  };
  const host = hostFixture('ABC');
  host.updates.apply({ mode: 'exact-edit', source: 'prior' });
  host.updates.apply({ mode: 'exact-edit', source: 'other' });
  assert.equal(undo(host.transport), true);
  assert.deepEqual([undoDepth(host.state()), redoDepth(host.state())], [1, 1]);
  let nested: unknown;
  const body = {};
  Object.assign(host.transport, {
    composing: true,
    compositionStarted: true,
    hasFocus: true,
    root: { activeElement: body },
    dom: { ownerDocument: { body } },
    contentDOM: {
      blur() {
        host.transport.composing = false;
        host.transport.compositionStarted = false;
        nested = host.updates.apply({
          mode: 'exact-edit',
          source: 'X\r\n🙂',
          configuration: { readOnly: false, placeholder: 'later' },
          selection: { anchor: 5, head: 1 },
        });
      },
    },
  });
  const composition = new module.CompositionHistory(host.transport as unknown as EditorView);
  Object.assign(host.transport, {
    plugin: (plugin: unknown) =>
      plugin === module.nativeCompositionHistory
        ? composition
        : { hasCompositionInput: () => false, cancelCompositionInput() {} },
  });
  const dispatch = host.transport.dispatch.bind(host.transport);
  host.transport.dispatch = (value) => {
    const transaction = value instanceof Transaction ? value : host.state().update(value);
    dispatch(transaction);
    composition.update({ transactions: [transaction] });
  };
  composition.start();
  try {
    assert.deepEqual(
      host.updates.apply({
        mode: 'preserve',
        configuration: { readOnly: true },
      }),
      { status: 'rejected', reason: 'superseded' },
    );
    assert.deepEqual(nested, { status: 'applied' });
    assert.equal(host.source() === 'X\r\n🙂', true);
    assert.equal(host.state().readOnly, false);
    assert.equal(host.permission.matches(false), true);
    assert.equal(host.configuration.values.placeholder, 'later');
    assert.deepEqual(
      [host.state().selection.main.anchor, host.state().selection.main.head],
      [4, 1],
    );
    assert.deepEqual([undoDepth(host.state()), redoDepth(host.state())], [2, 0]);
    assert.equal(undo(host.transport), true);
    assert.equal(host.source() === 'prior', true);
    assert.equal(undo(host.transport), true);
    assert.equal(host.source() === 'ABC', true);
    assert.equal(redo(host.transport), true);
    assert.equal(redo(host.transport), true);
    assert.equal(host.source() === 'X\r\n🙂', true);
  } finally {
    composition.destroy();
  }
});

test.after(() => hook.deregister());
