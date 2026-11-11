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
/** Actual search, surface and command owners share controlled public view publication and measurement transport. */
import assert from 'node:assert/strict';
import { Transaction, type TransactionSpec } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import {
  NativeSurface,
  type NativeViewFactory,
} from '../../../../../../frontend/sugarprompt/editor/src/native-surface.ts';
import { NativeSearch } from '../../../../../../frontend/sugarprompt/editor/src/native-search.ts';
import { nativeSourceCommands } from '../../../../../../frontend/sugarprompt/editor/src/native-source-commands.ts';
import { historyTransactions } from '../../../../../../frontend/sugarprompt/editor/src/prompt-history.ts';
import { changesExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { hostFixture } from '../transaction/host-document-fixture.ts';

/** Real owners decide admission and readiness; the transport records only public focus, dispatch and native measurements. */
export function searchSurfaceFixture() {
  const body = {},
    content = {},
    root = { activeElement: body as object };
  const measurements: Array<() => void> = [];
  let revision = 0,
    focuses = 0,
    composing = false;
  const refreshes: unknown[] = [];
  const create: NativeViewFactory = (_parent, initial) => {
    let state = initial;
    const view = {
      root,
      dom: { contains: (value: unknown) => value === content },
      scrollDOM: { clientHeight: 50 },
      get state() {
        return state;
      },
      get hasFocus() {
        return root.activeElement === content;
      },
      get composing() {
        return composing;
      },
      get compositionStarted() {
        return composing;
      },
      plugin: () => null,
      /** Publish real transactions before the controller-style passive observer runs. */
      dispatch(input: Transaction | TransactionSpec) {
        const transaction = input instanceof Transaction ? input : state.update(input);
        state = historyTransactions([transaction]).at(-1)!.state;
        if (changesExactSource(transaction)) revision++;
        owner?.observe();
      },
      /** Adopt only the surface's prepared native state. */
      setState(next: typeof state) {
        state = next;
      },
      /** Record the supported focus operation against the controlled destination. */
      focus() {
        root.activeElement = content;
        focuses++;
      },
      scrollSnapshot: () => ({}),
      /** Consume geometry only through the supported native read/write phases. */
      requestMeasure(request: NonNullable<Parameters<EditorView['requestMeasure']>[0]>) {
        measurements.push(() =>
          request.write?.(
            request.read(view as unknown as EditorView),
            view as unknown as EditorView,
          ),
        );
      },
      destroy() {},
    };
    return view as unknown as EditorView;
  };
  const surface = new NativeSurface(
    { ownerDocument: { body } } as unknown as HTMLElement,
    hostFixture('alpha').state(),
    () => {},
    create,
  );
  const owner = new NativeSearch(
    surface,
    () => revision,
    (request) => refreshes.push(request),
    () => {},
  );
  const search = owner;
  const commands = nativeSourceCommands(
    () => surface.view,
    () => undefined,
    () => {},
    {
      validate: (handoff) => surface.currentHandoff(handoff),
      accept: (admission, handoff) => {
        if (admission === 'selection') surface.retireHandoff('preserve-input');
        else assert.equal(surface.admitSource(handoff), true);
      },
    },
  );
  return {
    surface,
    search,
    commands,
    root,
    body,
    refreshes,
    counts: () => ({ focuses, queuedMeasurements: measurements.length }),
    /** Change the observed native composition property without fabricating source or search state. */
    composition(value: boolean) {
      composing = value;
      search.observe();
    },
    /** Deliver only actual scheduled native measurements and their publication microtasks. */
    async flush() {
      let passes = 0;
      while (measurements.length) {
        assert.ok(++passes <= 16, 'FINITE_MEASUREMENT_BOUND');
        measurements.shift()!();
        await Promise.resolve();
      }
    },
    /** Close command, search and canonical surface owners in dependency order. */
    dispose() {
      commands.dispose();
      search.dispose();
      surface.dispose();
    },
  };
}
