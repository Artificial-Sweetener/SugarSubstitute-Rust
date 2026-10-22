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
/** Current pointer intent selects precompiled structural feedback without source traversal or managed-DOM mutation. */
import { StateEffect, StateField, type Extension } from '@codemirror/state';
import { Decoration, EditorView } from '@codemirror/view';
import { editorFreshness } from './freshness.js';
import { richProjection } from './rich-projection.js';
import type { RegionTarget } from './region-index.js';

/** Source-bound intent retains the actual canonical target, including its scope-local ordinal. */
interface RegionHover {
  readonly source: symbol;
  readonly target: RegionTarget;
}
/** Native intent publishes one current ordinal rather than duplicating its prepared rendering. */
const setRegionHover = StateEffect.define<RegionHover | null>();
/** Source changes immediately retire feedback; semantic repaint cannot reanimate the old pointer intent. */
export const regionHover = StateField.define<RegionHover | null>({
  /** A new editor starts without pointer authority. */
  create: () => null,
  /** Bound pointer publication to current rich source and retire it on any source replacement. */
  update(current, transaction) {
    const source = transaction.state.field(editorFreshness).source;
    const projection = transaction.state.field(richProjection);
    if (
      current?.source !== source ||
      projection.sourceIdentity !== source ||
      projection.regionHovers[current.target.scopeId]?.[current.target.ordinal]?.target !==
        current.target
    )
      current = null;
    for (const effect of transaction.effects)
      if (
        effect.is(setRegionHover) &&
        (!effect.value ||
          (effect.value.source === source &&
            projection.sourceIdentity === source &&
            projection.regionHovers[effect.value.target.scopeId]?.[effect.value.target.ordinal]
              ?.target === effect.value.target))
      )
        current = effect.value;
    return current;
  },
  provide: (field) =>
    EditorView.decorations.compute([field, richProjection], (state) => {
      const hover = state.field(field);
      return hover
        ? (state.field(richProjection).regionHovers[hover.target.scopeId]?.[hover.target.ordinal]
            ?.decorations ?? Decoration.none)
        : Decoration.none;
    }),
});
/** Native pointer events perform bounded DOM ancestry and ordinal lookup, with no mutable parallel hover cache. */
export const regionHoverInteractions: Extension = [
  regionHover,
  EditorView.domEventHandlers({
    /** A divider hover accents its prepared partition, leaving label text color intact. */
    mousemove(event, view) {
      const row =
        event.target instanceof Element
          ? event.target.closest<HTMLElement>('.sp-region-separator')
          : null;
      const ordinal = row ? Number(row.dataset.regionIndex) : -1;
      const scopeId = row?.dataset.regionScope;
      const projection = view.state.field(richProjection);
      const source = view.state.field(editorFreshness).source;
      const target = scopeId ? projection.regionHovers[scopeId]?.[ordinal]?.target : undefined;
      const next =
        Number.isSafeInteger(ordinal) &&
        ordinal >= 0 &&
        projection.sourceIdentity === source &&
        target
          ? { source, target }
          : null;
      const current = view.state.field(regionHover);
      if (current?.target !== next?.target || current?.source !== next?.source)
        view.dispatch({ effects: setRegionHover.of(next) });
      return false;
    },
    /** Leaving the native surface retires its finite pointer intent. */
    mouseleave(_event, view) {
      if (view.state.field(regionHover)) view.dispatch({ effects: setRegionHover.of(null) });
      return false;
    },
  }),
];
