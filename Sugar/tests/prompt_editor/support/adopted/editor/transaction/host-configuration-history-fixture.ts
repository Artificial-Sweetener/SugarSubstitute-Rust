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
/** One finite actual native host/composition transport owns test-only scheduling and permission lifetime. */
import test from 'node:test';
import { registerHooks } from 'node:module';
import { readFileSync } from 'node:fs';
import { Transaction } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';

const observed = new URL(
  '../../../../../../frontend/sugarprompt/editor/src/composition-history.ts',
  import.meta.url,
).href;
const hook = registerHooks({
  /** Export-only observation exposes the genuine mounted cancellation owner without replacing its implementation. */
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
const composition = (await import(
  '../../../../../../frontend/sugarprompt/editor/src/composition-history.ts'
)) as unknown as {
  CompositionHistory: new (view: EditorView) => { destroy(): void };
  nativeCompositionHistory: unknown;
};
test.after(() => hook.deregister());

/** Finite headless transport dispatches actual CM transactions and the genuine permission cancellation owner. */
export function fixture(source = '') {
  const host = hostFixture(source);
  const body = {};
  Object.assign(host.transport, {
    root: { activeElement: body },
    dom: { ownerDocument: { body } },
    hasFocus: false,
  });
  const cancellation = new composition.CompositionHistory(host.transport as unknown as EditorView);
  Object.assign(host.transport, {
    plugin: (plugin: unknown) =>
      plugin === composition.nativeCompositionHistory ? cancellation : null,
  });
  let time = 1_000_000;
  return {
    host,
    /** Actual native timestamps stay within the donor's word idle window. */
    type(letter: string) {
      const state = host.state(),
        head = state.selection.main.head;
      host.transport.dispatch(
        state.update({
          changes: { from: head, insert: letter },
          selection: { anchor: head + letter.length },
          userEvent: 'input.type',
          annotations: Transaction.time.of(time++),
        }),
      );
    },
    /** Both real owners release their finite retained values. */
    destroy() {
      cancellation.destroy();
      host.destroy();
    },
  };
}
