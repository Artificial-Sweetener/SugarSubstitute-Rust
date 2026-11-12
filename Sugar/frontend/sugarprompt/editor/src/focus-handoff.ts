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
/** Native panels delegate source-free focus restoration to the current mounted surface. */
import { Facet } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import type { FocusHandoff } from './focus-handoff-contracts.js';

/** Panel replies require original intent; input restoration and reserved source ranges may inherit source-neutral selection. The source capability owner still validates reserved edits. */
export type FocusHandoffPurpose = 'panel' | 'input' | 'reserved-source';

/** One surface owns capture and freshness; native collaborators only delegate current intent admission. */
export interface NativeFocusIntent {
  /** Capture before callbacks can choose newer intent. */
  capture(purpose: FocusHandoffPurpose): FocusHandoff;
  /** Observe current intent without consuming it or changing any destination. */
  current(handoff: FocusHandoff): boolean;
  /** Focus an admitted pointer invocation without consuming its later one-shot action restoration. */
  refocus(handoff: FocusHandoff): Promise<boolean>;
}
/** Exactly one mounted surface supplies intent admission; headless state has no focus behavior. */
export const nativeFocusHandoff = Facet.define<NativeFocusIntent, NativeFocusIntent | null>({
  combine: (values) => {
    if (values.length > 1) throw new Error('Multiple native focus owners');
    return values[0] ?? null;
  },
});
/** Capture before dispatch/callback, never manufacture an intent after host publication. */
export function captureFocusHandoff(
  view: EditorView,
  purpose: FocusHandoffPurpose = 'panel',
): FocusHandoff {
  return (
    view.state.facet(nativeFocusHandoff)?.capture(purpose) ?? {
      restore: () => Promise.resolve(false),
      cancel: () => undefined,
    }
  );
}
/** Source commands validate the original handoff against the surface owner before publication; headless state has no destination to supersede. */
export function currentFocusHandoff(view: EditorView, handoff: FocusHandoff): boolean {
  return view.state.facet(nativeFocusHandoff)?.current(handoff) ?? true;
}

/** The mounted surface alone admits original pointer focus; the same capability remains live for menu cancellation or action transfer. */
export function refocusCapturedHandoff(view: EditorView, handoff: FocusHandoff): Promise<boolean> {
  return view.state.facet(nativeFocusHandoff)?.refocus(handoff) ?? Promise.resolve(false);
}
