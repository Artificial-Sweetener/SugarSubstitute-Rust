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
/** Actual CM host/configuration/composition owners run behind inert DOM listeners and view dispatch only. */
import { EditorSelection, StateEffect, Transaction, type TransactionSpec } from '@codemirror/state';
import type { EditorView, ViewUpdate } from '@codemirror/view';
import { fixture as compositionFixture } from '../transaction/host-configuration-history-fixture.ts';
import { NativeContextMenu } from '../../../../../../frontend/sugarprompt/editor/src/context-menu.ts';
import { nativeFocusHandoff } from '../../../../../../frontend/sugarprompt/editor/src/focus-handoff.ts';
import { spellingProvider } from '../../../../../../frontend/sugarprompt/editor/src/spelling-provider.ts';
import type { FieldActionEntry } from '../../../../../../frontend/sugarprompt/editor/src/field-actions-contracts.ts';

/** Listeners/focus are controlled transport; source, selection, configuration, history and menu admission are production owners. */
export function fieldFixture() {
  const native = compositionFixture('😀first\r\nsecond'),
    host = native.host,
    listeners = new Set<unknown>();
  const window = {
    addEventListener(_name: string, callback: unknown) {
      listeners.add(callback);
    },
    removeEventListener(_name: string, callback: unknown) {
      listeners.delete(callback);
    },
  };
  const document = {
    defaultView: window,
    addEventListener(_name: string, callback: unknown) {
      listeners.add(callback);
    },
    removeEventListener(_name: string, callback: unknown) {
      listeners.delete(callback);
    },
  };
  Object.assign(host.transport, { dom: { ownerDocument: document } });
  let captures = 0,
    restores = 0,
    cancels = 0,
    focusCurrent = true;
  let cancelFailure: unknown, spellingFailure: unknown;
  const focus = {
    async restore() {
      restores++;
      return focusCurrent;
    },
    cancel() {
      cancels++;
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      if (cancelFailure !== undefined) throw cancelFailure;
    },
  };
  host.transport.dispatch({
    effects: StateEffect.appendConfig.of(
      nativeFocusHandoff.of({
        capture() {
          captures++;
          return focus;
        },
        current(value) {
          return value === focus && focusCurrent;
        },
        /** Controlled field transport delegates admitted focus without a second destination policy. */
        refocus(value) {
          return value === focus && focusCurrent ? focus.restore() : Promise.resolve(false);
        },
      }),
    ),
  });
  const originalPlugin = host.transport.plugin as (plugin: unknown) => unknown;
  Object.assign(host.transport, {
    plugin: (plugin: unknown) =>
      plugin === spellingProvider && spellingFailure !== undefined
        ? {
            cancelSuggestions() {
              throw spellingFailure;
            },
          }
        : originalPlugin(plugin),
  });
  const owner = new NativeContextMenu(host.transport as unknown as EditorView, (enabled) => {
    host.updates.apply({
      mode: 'preserve',
      configuration: { richRendering: enabled },
    });
  });
  const dispatch = host.transport.dispatch.bind(host.transport);
  let publication: (() => void) | undefined;
  Object.assign(host.transport, {
    /** Deliver genuine pre/post transaction states through the established view-update seam. */
    dispatch(input: Transaction | TransactionSpec) {
      const before = host.state(),
        transaction = input instanceof Transaction ? input : before.update(input);
      dispatch(transaction);
      owner.update({
        startState: before,
        state: host.state(),
        transactions: [transaction],
        viewportChanged: false,
      } as unknown as ViewUpdate);
      publication?.();
    },
  });
  host.transport.dispatch({ selection: EditorSelection.single(6, 2) });
  return {
    host,
    owner,
    focus,
    listeners,
    counts: () => ({ captures, restores, cancels }),
    onPublication(callback: (() => void) | undefined) {
      publication = callback;
    },
    failFocusCancel(cause: unknown) {
      cancelFailure = cause;
    },
    failSpellingCancel(cause: unknown) {
      spellingFailure = cause;
    },
    supersedeFocus() {
      focusCurrent = false;
    },
    destroy() {
      owner.destroy();
      native.destroy();
    },
  };
}
/** Locate the real issued descriptor by literal semantic label, including nested scopes. */
export function fieldAction(
  entries: readonly FieldActionEntry[],
  label: string,
): Extract<FieldActionEntry, { kind: 'action' }> {
  for (const entry of entries) {
    if (entry.kind === 'action' && entry.label === label) return entry;
    if (entry.kind === 'submenu') {
      const nested = entry.children.find((item) => item.kind === 'action' && item.label === label);
      if (nested?.kind === 'action') return nested;
    }
  }
  throw new Error('Expected prepared field action');
}
