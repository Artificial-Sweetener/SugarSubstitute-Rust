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
/** Actual search and focus owners keep explicit match scrolling independent from native selection. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, type TransactionSpec } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { history, undoDepth } from '@codemirror/commands';
import { NativeSearch } from '../../../../../../frontend/sugarprompt/editor/src/native-search.ts';
import { NativeFocus } from '../../../../../../frontend/sugarprompt/editor/src/native-focus.ts';
import type { NativeSurface } from '../../../../../../frontend/sugarprompt/editor/src/native-surface.ts';
import {
  exactSourceExtension,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { searchMatches } from '../../../../../../frontend/sugarprompt/editor/src/search-matches.ts';

/** The finite transport consumes actual nonempty requests through CodeMirror's public measurement method. */
type NativeMeasure = NonNullable<Parameters<EditorView['requestMeasure']>[0]>;

test('nonselecting match navigation never replaces its explicit scroll target with the unchanged caret', async () => {
  let state = EditorState.create({
    doc: 'alpha',
    extensions: [exactSourceExtension('alpha'), editorFreshness, searchMatches, history()],
  });
  const measures: Array<() => void> = [],
    targets: number[] = [];
  let focused = false;
  // This exact external boundary supplies finite DOM/focus/measurement transport; real owners decide effects and readiness.
  const transport = {
    get state() {
      return state;
    },
    compositionStarted: false,
    root: { activeElement: null },
    dom: { contains: () => false },
    scrollDOM: { clientHeight: 30 },
    get hasFocus() {
      return focused;
    },
    focus() {
      focused = true;
    },
    dispatch(spec: TransactionSpec) {
      state = state.update(spec).state;
    },
    requestMeasure(request: NativeMeasure) {
      measures.push(() => request.write?.(request.read(view), view));
    },
  };
  const view = transport as unknown as EditorView,
    focus = new NativeFocus(view, { set: () => 1, clear: () => undefined });
  // Only these existing native-surface operations are consumed; this adapter owns no source or focus policy.
  const surface = {
    view,
    retireHandoff: () => focus.retire(),
    focus: (viewport: 'reveal-selection' | 'preserve-viewport', signal?: AbortSignal) =>
      focus.focus(viewport, signal),
  } as unknown as NativeSurface;
  const owner = new NativeSearch(
      surface,
      () => 0,
      () => undefined,
      () => undefined,
    ),
    request = owner.setContext({ fieldId: 'field', queryId: 'query' })!;
  owner.publish({
    request,
    resultRevision: 1,
    matches: [{ matchId: 'match', from: 4, to: 5 }],
    activeIndex: null,
  });
  const original = EditorView.scrollIntoView;
  EditorView.scrollIntoView = (target, options) => {
    targets.push(typeof target === 'number' ? target : target.head);
    return original.call(EditorView, target, options);
  };
  try {
    let outcome: Awaited<ReturnType<typeof owner.navigate>> | undefined;
    const navigation = owner
      .navigate({
        request,
        resultRevision: 1,
        matchId: 'match',
        select: false,
        scroll: true,
        focus: true,
      })
      .then((result) => (outcome = result));
    // NativeFocus caps eight measurement passes; four additional turns admit and settle its search continuation.
    for (let pass = 0; pass < 12 && outcome === undefined; pass++) {
      await Promise.resolve();
      measures.shift()?.();
    }
    assert.deepEqual(outcome, { status: 'applied' });
    await navigation;
    assert.equal(measures.length, 0);
    assert.equal(state.selection.main.head, 0);
    assert.equal(readExactSource(state) === 'alpha', true);
    assert.equal(undoDepth(state), 0);
    assert.deepEqual(targets, [5]);
  } finally {
    EditorView.scrollIntoView = original;
    owner.dispose();
    focus.dispose();
  }
});
