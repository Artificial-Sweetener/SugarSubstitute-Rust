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
/** Real CodeMirror owners plus controlled input/focus transport expose synchronous host boundaries. */
import { EditorState, type TransactionSpec } from '@codemirror/state';
import { history } from '@codemirror/commands';
import {
  exactSourceExtension,
  changesExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { nativeInputSettlement } from '../../../../../../frontend/sugarprompt/editor/src/input-settlement.ts';
import { searchMatches } from '../../../../../../frontend/sugarprompt/editor/src/search-matches.ts';
import { NativeSearch } from '../../../../../../frontend/sugarprompt/editor/src/native-search.ts';
import type { NativeSurface } from '../../../../../../frontend/sugarprompt/editor/src/native-surface.ts';
import type {
  SearchHostIntent,
  SearchRequest,
  SearchResults,
} from '../../../../../../frontend/sugarprompt/services/src/search/contracts.ts';

/** Hooks represent external callbacks, never another source, input or geometry owner. */
export interface SearchOwnerHooks {
  retire?: () => void;
  dispatch?: () => void;
  focus?: () => void;
  intent?: (intent: SearchHostIntent) => void;
}

/** Native state owns source/history; the supported input facet supplies controlled composition observation. */
export function searchOwnerFixture(holdFocus = false) {
  let composing = false,
    revision = 0,
    settleFocus: ((ready: boolean) => void) | null = null;
  const hooks: SearchOwnerHooks = {},
    focuses: string[] = [],
    refreshes: SearchRequest[] = [];
  let state = EditorState.create({
    doc: 'alpha',
    extensions: [
      exactSourceExtension('alpha'),
      editorFreshness,
      searchMatches,
      history(),
      nativeInputSettlement.of({
        hasCompositionInput: () => composing,
        cancelCompositionInput() {},
        hasHeldKey: () => false,
        reserveCommand: () => ({ finish() {}, cancel() {} }),
      }),
    ],
  });
  const view = {
    get state() {
      return state;
    },
    compositionStarted: false,
    dispatch(spec: TransactionSpec) {
      const transaction = state.update(spec);
      state = transaction.state;
      if (changesExactSource(transaction)) revision++;
      hooks.dispatch?.();
    },
  };
  const surface = {
    view,
    retireHandoff() {
      const settle = settleFocus;
      settleFocus = null;
      settle?.(false);
      hooks.retire?.();
    },
    focus(viewport: string, signal?: AbortSignal) {
      focuses.push(viewport);
      let settle: ((ready: boolean) => void) | undefined;
      const result = holdFocus
        ? new Promise<boolean>((resolve) => {
            settle = resolve;
            settleFocus = resolve;
          })
        : Promise.resolve(true);
      /** An older cancelled seam request cannot settle a newer controlled focus. */
      const cancel = () => {
        if (settleFocus === settle) settleFocus = null;
        settle?.(false);
      };
      signal?.addEventListener('abort', cancel, { once: true });
      void result.finally(() => signal?.removeEventListener('abort', cancel));
      if (signal?.aborted) cancel();
      hooks.focus?.();
      return result;
    },
  };
  const owner = new NativeSearch(
    surface as unknown as NativeSurface,
    () => revision,
    (request) => refreshes.push(request),
    (intent) => hooks.intent?.(intent),
  );
  return {
    owner,
    hooks,
    focuses,
    refreshes,
    get state() {
      return state;
    },
    get pendingFocus() {
      return settleFocus !== null;
    },
    beginComposition(observe = true) {
      composing = true;
      if (observe) owner.observe();
    },
    endComposition() {
      composing = false;
      owner.observe();
    },
    change(spec: TransactionSpec) {
      view.dispatch(spec);
      owner.observe();
    },
  };
}

/** Independent host specimen identifies one authored exact UTF16 match. */
export function searchPublication(request: SearchRequest, resultRevision = 1): SearchResults {
  return {
    request,
    resultRevision,
    matches: [{ matchId: 'match', from: 0, to: 5 }],
    activeIndex: null,
  };
}

/** Independent explicit navigation intent requests match selection, native scroll and focus. */
export function searchNavigation(request: SearchRequest) {
  return {
    request,
    resultRevision: 1,
    matchId: 'match',
    select: true,
    scroll: true,
    focus: true,
  };
}
