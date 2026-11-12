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
/** Current pointer or host-linked intent selects precompiled structural feedback without source traversal or managed-DOM mutation. */
import {
  Facet,
  StateEffect,
  StateField,
  type EditorState,
  type Extension,
} from '@codemirror/state';
import { Decoration, EditorView } from '@codemirror/view';
import { editorFreshness } from './freshness.js';
import { richProjection } from './rich-projection.js';
import { documentKind } from './document-kind.js';
import type { RegionTarget } from './region-index.js';

/** Source-bound intent retains the actual canonical target, including its scope-local ordinal. */
interface RegionHover {
  readonly source: symbol;
  readonly target: RegionTarget;
  /** Set by the host to mirror another view's hover; pointer intent omits it. */
  readonly linked?: true;
}
/** One host observer receives pointer hover of a prompt document's separator ordinals. */
export const regionHoverObserver = Facet.define<
  (ordinal: number | null) => void,
  ((ordinal: number | null) => void) | null
>({
  /** Duplicate publication owners are observable rather than silently selected. */
  combine(values) {
    if (values.length > 1) throw new Error('Multiple region hover observers');
    return values[0] ?? null;
  },
});
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
      pointRegionSeparator(
        view,
        event.target instanceof Element
          ? event.target.closest<HTMLElement>('.sp-region-separator')
          : null,
      );
      return false;
    },
    /** Leaving the native surface retires its finite pointer intent. */
    mouseleave(_event, view) {
      pointRegionSeparator(view, null);
      return false;
    },
  }),
];

/** Point at a rendered separator row, or at no separator, and report the change to the host. */
export function pointRegionSeparator(
  view: Pick<EditorView, 'state' | 'dispatch'>,
  row: Pick<HTMLElement, 'dataset'> | null,
): void {
  const ordinal = row ? Number(row.dataset.regionIndex) : -1;
  const scopeId = row?.dataset.regionScope;
  const projection = view.state.field(richProjection);
  const source = view.state.field(editorFreshness).source;
  const target = scopeId ? projection.regionHovers[scopeId]?.[ordinal]?.target : undefined;
  const next =
    Number.isSafeInteger(ordinal) && ordinal >= 0 && projection.sourceIdentity === source && target
      ? { source, target }
      : null;
  const current = view.state.field(regionHover);
  if (current?.target === next?.target && current?.source === next?.source) return;
  view.dispatch({ effects: setRegionHover.of(next) });
  if (next || (current && !current.linked)) publishPointerHover(view.state, next?.target ?? null);
}

/**
 * Mirror another view's hover on separator `ordinal` of a prompt document, or clear it with null.
 *
 * Pointer hover is never replaced or cleared by the host, and host-linked hover is never reported
 * to the observer. A source edit retires linked hover like pointer hover.
 *
 * @returns Whether the separator is now shown hovered.
 */
export function showLinkedRegionHover(
  view: Pick<EditorView, 'state' | 'dispatch'>,
  ordinal: number | null,
): boolean {
  if (ordinal !== null && (!Number.isSafeInteger(ordinal) || ordinal < 0))
    throw new TypeError('Invalid region ordinal');
  const current = view.state.field(regionHover);
  if (current && !current.linked) return current.target === promptRegionTarget(view.state, ordinal);
  const target = promptRegionTarget(view.state, ordinal);
  if ((current?.target ?? null) === target) return target !== null;
  view.dispatch({
    effects: setRegionHover.of(
      target ? { source: view.state.field(editorFreshness).source, target, linked: true } : null,
    ),
  });
  return target !== null;
}

/** Resolve a prompt document's separator target by ordinal; other documents have no peers. */
function promptRegionTarget(state: EditorState, ordinal: number | null): RegionTarget | null {
  if (ordinal === null || state.facet(documentKind) !== 'prompt') return null;
  const projection = state.field(richProjection),
    scopes = Object.values(projection.regionHovers);
  if (projection.sourceIdentity !== state.field(editorFreshness).source || scopes.length !== 1)
    return null;
  return scopes[0]?.[ordinal]?.target ?? null;
}

/** Report pointer hover of a prompt document's separator to the host observer. */
function publishPointerHover(state: EditorState, target: RegionTarget | null): void {
  if (state.facet(documentKind) !== 'prompt') return;
  state.facet(regionHoverObserver)?.(target?.ordinal ?? null);
}
