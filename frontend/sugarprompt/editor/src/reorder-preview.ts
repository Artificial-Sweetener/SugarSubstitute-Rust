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
/** A finite noneditable preview replaces only one genuine value; native source and caret remain untouched. */
import { requireValue } from '../../core/src/required.js';
import { snapshotPresentation } from './native-snapshot-policy.js';
import { StateEffect, StateField, Prec, Transaction, type Extension } from '@codemirror/state';
import { Decoration, EditorView, type DecorationSet } from '@codemirror/view';
import { currentNativeReorder } from './reorder-command.js';
import type {
  ReorderPreview,
  ReorderPublicationOutcome,
  ReorderPointerPaint,
} from './reorder-preview-contracts.js';
import type { ReorderApplicationHold } from './reorder-session-application.js';
import { ReorderWidget } from './reorder-widget.js';
import { prepareReorderWidgetPlan } from './reorder-widget-plan.js';
import { nativeReorderRecipe, nativeReorderRecipeDecorations } from './reorder-native-recipe.js';
import { semanticStructures } from './structure.js';
import { nativeReorderDisplay } from './reorder-native-display.js';
import {
  setReorderMotionMask,
  releaseReorderMotionMask,
  type ReorderMotionMask,
} from './reorder-motion-mask.js';
import { reorderPaintIssueName } from './reorder-paint-identity.js';
import { prepareReorderStoragePaint } from './reorder-storage-paint.js';
import {
  preparedReorderPreview,
  prepareReorderPreview,
  type ReorderPreviewInput,
  sameReorderOrder,
  samePreparedReorderPaint,
} from './reorder-preview-preparation.js';
/** Publication and retirement use the same finite session identity. */
export const installReorderPreview = StateEffect.define<ReorderPreview | null>();
/** The threshold-admitted Session supplies the actual immutable primary pickup operand. */
const installPointerPaint = StateEffect.define<ReorderPointerPaint>();
/** Only the exact active hold can relinquish complete passive presentation. */
const fallbackPointerPaint = StateEffect.define<{
  readonly pointer: ReorderPointerPaint;
  readonly preview: ReorderPreview;
}>();
/** Publish correct current CM proposal before its external whole-value coverage can retire. */
export function fallbackReorderPointerPaint(
  view: EditorView,
  pointer: ReorderPointerPaint,
  preview: ReorderPreview,
  current: () => boolean,
): boolean {
  if (!current()) return false;
  const state = view.state.field(reorderPreview, false);
  if (state?.pointer !== pointer || state.value !== preview || pointer.mode !== 'stage')
    return false;
  view.dispatch({
    effects: fallbackPointerPaint.of({ pointer, preview }),
    annotations: [Transaction.addToHistory.of(false), snapshotPresentation.of(true)],
  });
  return (
    current() &&
    view.state.field(reorderPreview).value === preview &&
    view.state.field(reorderPreview).pointer?.hold === pointer.hold &&
    view.state.field(reorderPreview).pointer?.mode === 'fallback'
  );
}
/** Only these exact effects publish ephemeral reorder display without changing native input geometry authority. */
export function reorderPresentationEffect(effect: StateEffect<unknown>): boolean {
  return (
    effect.is(installReorderPreview) ||
    effect.is(installPointerPaint) ||
    effect.is(fallbackPointerPaint) ||
    effect.is(setReorderMotionMask) ||
    effect.is(releaseReorderMotionMask)
  );
}
/** Publish source-neutral paint under the gesture's currentness authority; a new sticky-canonical hold prepares its exact future anchor plan before CM can reuse equivalent mounted output. */
export function publishReorderPreview(
  view: EditorView,
  input: ReorderPreviewInput,
  presentation: 'complete' | 'pointer',
  hold: ReorderApplicationHold | null,
  current: () => boolean,
): ReorderPublicationOutcome {
  if (!current() || !input.order) return 'stale';
  const preview = prepareReorderPreview(input, presentation);
  if (!preview) return 'unavailable';
  const publicationState = view.state,
    publication = publicationState.field(reorderPreview, false),
    previous = publication?.pointer;
  let preparedAnchor: ReorderPreview | null = null;
  let pointer: ReorderPointerPaint | null = null;
  if (
    presentation === 'pointer' &&
    hold &&
    previous?.hold !== hold &&
    !sameReorderOrder(input.order, hold.order)
  ) {
    const anchor = prepareReorderPreview({ ...input, order: hold.order }, 'pointer');
    if (!anchor) return 'unavailable';
    if (publication?.value && !publication.native) {
      // CM may retain equivalent pickup paint without mounting this exact anchor publication.
      prepareReorderWidgetPlan(anchor);
      if (view.state !== publicationState || !current() || view.state !== publicationState)
        return 'stale';
      preparedAnchor = anchor;
    }
    pointer = Object.freeze({ hold, anchor, mode: 'stage' });
  }
  view.dispatch({
    effects: [
      installReorderPreview.of(preview),
      ...(pointer ? [installPointerPaint.of(pointer)] : []),
    ],
    annotations: [Transaction.addToHistory.of(false), snapshotPresentation.of(true)],
  });
  if (preparedAnchor && view.state.field(reorderPreview, false)?.paint !== preparedAnchor)
    return 'stale';
  // Native outer marks can reparent selected text; synchronize only current focused native paint.
  if (current() && view.hasFocus && view.state.field(reorderPreview).native) view.focus();
  return current() ? 'published' : 'stale';
}
/** Current publication holds one derived decoration set and no alternate source. */
interface PreviewState {
  readonly value: ReorderPreview | null;
  /** The actual CM Widget/outer marks may remain at a primary hold's immutable pickup order. */
  readonly paint: ReorderPreview | null;
  readonly pointer: ReorderPointerPaint | null;
  readonly decorations: DecorationSet;
  readonly outer: DecorationSet;
  /** Unchanged paint retains native lines and canonical widgets instead of remounting a derived layout. */
  readonly native: boolean;
  /** Visibility belongs to this exact preview and retires atomically with its source admission. */
  readonly mask: ReorderMotionMask | null;
}
const empty: PreviewState = Object.freeze({
  value: null,
  paint: null,
  pointer: null,
  decorations: Decoration.none,
  outer: Decoration.none,
  native: false,
  mask: null,
});
/** Source/context/selection changes retire the session's rendering immediately. */
export const reorderPreview = StateField.define<PreviewState>({
  /** There is no preview before a finite command enters. */
  create: () => empty,
  /** A genuine scope is the only replaceable source extent. */
  update(value, transaction) {
    let next = value.value;
    let mask = value.mask;
    let pointer = value.pointer;
    for (const effect of transaction.effects) {
      if (effect.is(installReorderPreview)) next = effect.value;
      else if (effect.is(installPointerPaint)) pointer = effect.value;
      else if (
        effect.is(fallbackPointerPaint) &&
        pointer === effect.value.pointer &&
        next === effect.value.preview
      )
        pointer = Object.freeze({ ...pointer, mode: 'fallback' });
      else if (effect.is(setReorderMotionMask)) {
        const candidate = effect.value;
        if (
          candidate.ids.length > 64 ||
          new Set(candidate.ids).size !== candidate.ids.length ||
          candidate.ids.some(
            (id) => !Number.isSafeInteger(id) || id < 0 || !candidate.capture.prepared.chips[id],
          )
        )
          throw new TypeError('reorder-motion-mask-invalid');
        mask = Object.freeze({
          ...candidate,
          ids: Object.freeze([...candidate.ids]),
        });
      } else if (effect.is(releaseReorderMotionMask) && mask?.issue === effect.value) mask = null;
    }
    if (!next || !currentNativeReorder(transaction.state, next.capture)) return empty;
    if (
      next.presentation !== 'pointer' ||
      pointer?.anchor.token !== next.token ||
      pointer.anchor.capture !== next.capture ||
      pointer.anchor.activeChip !== next.activeChip
    )
      pointer = null;
    if (mask?.capture !== next.capture) mask = null;
    if (next === value.value && mask === value.mask && pointer === value.pointer) return value;
    const paint = pointer?.mode === 'stage' ? pointer.anchor : next;
    const paintMask = pointer?.mode === 'stage' || mask?.coverage === 'value' ? null : mask;
    const previousPaintMask =
      value.pointer?.mode === 'stage' || value.mask?.coverage === 'value' ? null : value.mask;
    const range = prepareReorderStoragePaint(paint).replacement;
    const native =
      preparedReorderPreview(paint) &&
      (paint.presentation === 'complete' || paint.presentation === 'pointer') &&
      sameReorderOrder(paint.order, paint.capture.prepared.initial) &&
      !(
        value.value?.token === next.token &&
        value.value.capture === next.capture &&
        preparedReorderPreview(value.value) &&
        !value.native
      );
    const nativeRecipe = native
      ? nativeReorderRecipe(paint.capture.context.values, paint.capture.target.entry)
      : null;
    if (native && (!nativeRecipe || nativeRecipe.prepared !== paint.capture.prepared))
      throw new Error('native-reorder-recipe-unavailable');
    const covered = !native && (pointer?.mode === 'stage' || mask?.coverage === 'value');
    const outer = covered
      ? (value.paint?.capture === paint.capture || mask?.coverage === 'value') &&
        value.outer !== Decoration.none
        ? value.outer
        : Decoration.set([
            Decoration.mark({ class: 'sp-reorder-pointer-underlay' }).range(
              transaction.state.doc.lineAt(range.from).from,
              transaction.state.doc.lineAt(range.to).to,
            ),
          ])
      : Decoration.none;
    return {
      value: next,
      paint,
      pointer,
      outer,
      native,
      mask,
      decorations:
        (paint === value.paint ||
          (native && !!value.paint && samePreparedReorderPaint(paint, value.paint))) &&
        paintMask === previousPaintMask &&
        native === value.native
          ? value.decorations
          : native
            ? requireValue(nativeRecipe, 'reorder-preview').decorations
            : Decoration.set([
                Decoration.replace({
                  widget: new ReorderWidget(paint, paintMask),
                }).range(range.from, range.to),
              ]),
    };
  },
  /** Native outer marks wrap existing rich widgets; changed-output replacements use the direct layout facet. */
  provide: (field) => [
    nativeReorderDisplay,
    Prec.highest(
      EditorView.decorations.from(field, (value) =>
        value.native ? Decoration.none : value.decorations,
      ),
    ),
    EditorView.outerDecorations.from(field, (value) => value.outer),
    EditorView.outerDecorations.compute(
      [semanticStructures, field, nativeReorderDisplay],
      (state) => {
        const publication = state.field(field),
          values = state.field(semanticStructures, false)?.native?.nativeValues;
        return publication.paint && !publication.native
          ? nativeReorderRecipeDecorations(values, publication.paint.capture.target.entry)
          : state.field(nativeReorderDisplay);
      },
    ),
  ],
});
/** Preview styles affect only owner-created nodes and keep exact separator whitespace visible. */
export const reorderPreviewExtension: Extension = [
  reorderPreview,
  EditorView.editorAttributes.compute([reorderPreview], (state) => {
    const preview = state.field(reorderPreview);
    const recipe =
      preview.native && preview.paint
        ? nativeReorderRecipe(
            preview.paint.capture.context.values,
            preview.paint.capture.target.entry,
          )
        : null;
    const nativeStyle =
      recipe && preview.paint
        ? '--sp-reorder-native-' +
          String(recipe.key) +
          '-target-display:inline-block;--sp-reorder-native-' +
          String(recipe.key) +
          '-target-visibility:visible;--sp-reorder-native-' +
          String(recipe.key) +
          '-target-events:auto;--sp-reorder-native-' +
          String(recipe.key) +
          '-cursor:grab;--sp-reorder-native-' +
          String(recipe.key) +
          '-hover-fill:rgba(0,0,0,.086);--sp-reorder-native-' +
          String(recipe.key) +
          '-outline:solid;--sp-reorder-native-' +
          String(recipe.key) +
          '-fill:rgba(0,0,0,.055);--sp-reorder-native-' +
          String(recipe.key) +
          '-' +
          String(preview.paint.activeChip) +
          '-outline:rgba(0,0,0,.549);--sp-reorder-native-' +
          String(recipe.key) +
          '-' +
          String(preview.paint.activeChip) +
          '-fill:rgba(0,0,0,.110)' +
          (preview.paint.presentation === 'pointer'
            ? ';--sp-reorder-native-' +
              String(recipe.key) +
              '-' +
              String(preview.paint.activeChip) +
              '-visibility:hidden'
            : '')
        : '';
    return {
      ...(preview.native ? { class: 'sp-reorder-preview' } : {}),
      ...(recipe && preview.paint
        ? {
            'data-reorder-native-value': String(recipe.key),
            'data-reorder-native-active': String(preview.paint.activeChip),
            ...(preview.paint.presentation === 'pointer'
              ? { 'data-reorder-native-held': 'true' }
              : {}),
            style: nativeStyle,
          }
        : {}),
      ...(preview.value ? { 'data-reorder-preview': 'true' } : {}),
      ...(preview.pointer
        ? {
            'data-reorder-pointer-paint':
              preview.pointer.mode === 'fallback'
                ? 'fallback'
                : preview.mask?.coverage === 'value'
                  ? 'stage'
                  : 'anchor',
          }
        : {}),
      ...(preview.mask
        ? {
            ...(preview.mask.coverage === 'value'
              ? { 'data-reorder-motion-coverage': 'value' }
              : {}),
            'data-reorder-motion': preview.mask.ids.join(' '),
            'data-reorder-motion-scope': reorderPaintIssueName(preview.mask.scope),
            'data-reorder-motion-issue': reorderPaintIssueName(preview.mask.issue),
          }
        : {}),
    };
  }),
  EditorView.theme({
    '&[data-reorder-preview="true"] .cm-content, &[data-reorder-preview="true"] .cm-content *': {
      caretColor: 'transparent',
    },
    '&[data-reorder-preview="true"] .cm-content::selection, &[data-reorder-preview="true"] .cm-content ::selection':
      { backgroundColor: 'transparent', color: 'inherit' },
    '.sp-reorder-preview': { whiteSpace: 'inherit', userSelect: 'none' },
    '.sp-reorder-chip': {
      display: 'inline',
      position: 'relative',
      border: '0',
      padding: '0',
      margin: '0',
      outline: '1px solid rgba(0, 0, 0, .322)',
      borderRadius: '9px',
      backgroundColor: 'rgba(0, 0, 0, .055)',
      cursor: 'grab',
      whiteSpace: 'inherit',
    },
    '.sp-reorder-chip[data-reorder-native-value]': {
      outlineStyle: 'var(--sp-native-outline,none)',
      outlineColor: 'var(--sp-native-active-outline,rgba(0,0,0,.322))',
      backgroundColor: 'var(--sp-native-active-fill,var(--sp-native-background,transparent))',
      visibility: 'var(--sp-native-visibility,visible)',
      cursor: 'var(--sp-native-cursor,inherit)',
    },
    '.sp-reorder-chip[data-reorder-native-value]:hover': {
      outlineColor: 'var(--sp-native-active-outline,rgba(0,0,0,.471))',
      backgroundColor: 'var(--sp-native-active-fill,var(--sp-native-hover-background,transparent))',
    },
    '.sp-reorder-gap': { whiteSpace: 'inherit' },
    '.sp-reorder-held': { visibility: 'hidden' },
    '.sp-reorder-chip:hover': {
      outlineColor: 'rgba(0, 0, 0, .471)',
      backgroundColor: 'rgba(0, 0, 0, .086)',
    },
    // A held chip keeps its active cue while native capture crosses hover and pointer-exit frames.
    '.sp-reorder-chip-active, &[data-reorder-grab="true"] .sp-reorder-chip-active:hover': {
      outlineColor: 'rgba(0, 0, 0, .549)',
      backgroundColor: 'rgba(0, 0, 0, .110)',
    },
    '.sp-reorder-empty': {
      display: 'inline-block',
      minWidth: '12px',
      minHeight: '1em',
      borderBottom: '1px dotted currentColor',
    },
    '.sp-reorder-native-empty': {
      position: 'relative',
    },
    '.sp-reorder-native-empty > .sp-reorder-empty': {
      position: 'absolute',
      left: '0',
      top: '0',
    },
    '.sp-reorder-proxy': {
      position: 'fixed',
      pointerEvents: 'none',
      zIndex: '100',
      whiteSpace: 'pre-wrap',
      border: '1px solid rgba(0, 0, 0, .580)',
      borderRadius: '9px',
      background: 'rgba(0, 0, 0, .118)',
      padding: '3px 5px',
    },
  }),
];
