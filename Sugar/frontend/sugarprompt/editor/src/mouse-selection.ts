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
/** Prompt-specific pointer intent delegates selection, focus, drag listeners and history to CodeMirror. */
import { EditorSelection, type SelectionRange } from '@codemirror/state';
import {
  EditorView,
  ViewPlugin,
  type MouseSelectionStyle,
  type ViewUpdate,
} from '@codemirror/view';
import { promptWordRange } from '../../core/src/prompt/selection.js';
import { featureEnabled } from '../../services/src/index.js';
import { semanticStructures } from './structure.js';
import { regionContentCaret } from './region-caret.js';
import { featureProfile } from './features.js';
import { richRendering } from './presentation.js';
import { currentInputProjection } from './rich-projection.js';
import { editorFreshness } from './freshness.js';
import {
  heldProjection,
  holdProjectionToken,
  releaseProjectionToken,
} from './projection-exposure.js';
import type { SourceRange } from '../../core/src/offsets.js';
/** A pointer hit keeps the side it landed on so a wrap boundary resolves to the clicked visual row. */
interface PointerHit {
  readonly pos: number;
  readonly assoc: -1 | 1;
}
/** A finite follow-up intent observes native selection; it owns no caret, source or document history. */
export class PromptMouseSelection {
  private pending: SelectionRange | null = null;
  private readonly view: EditorView;
  private held: symbol | null = null;
  private gesture: symbol | null = null;
  private disposed = false;
  private readonly document: Document;
  /** Passive termination changes only derived visibility; CodeMirror retains all gesture selection and focus actions. */
  private readonly terminate = (event: Event): void => {
    if (event.type === 'mousemove' && (event as MouseEvent).buttons !== 0) return;
    if (event.type !== 'mouseup') this.gesture = null;
    this.release();
  };
  /** Observe one native view for its mounted lifetime. */
  constructor(view: EditorView) {
    this.view = view;
    this.document = view.contentDOM.ownerDocument;
    try {
      for (const name of ['mouseup', 'mousemove', 'mousedown', 'pointercancel'])
        this.document.addEventListener(name, this.terminate, {
          passive: true,
          capture: name === 'mousedown',
        });
      this.document.defaultView?.addEventListener('blur', this.terminate);
    } catch (cause) {
      this.disposed = true;
      try {
        this.unobserve();
      } catch (cleanup) {
        throw new AggregateError(
          [cause, cleanup],
          'MOUSE_EXPOSURE_OBSERVER_ADMISSION_AND_CLEANUP_FAILED',
        );
      }
      throw cause;
    }
  }
  /** Retire intent on source, selection, profile or presentation changes. */
  update(update: ViewUpdate): void {
    if (
      update.startState.field(editorFreshness) !== update.state.field(editorFreshness) ||
      update.startState.facet(richRendering) !== update.state.facet(richRendering) ||
      update.transactions.some(
        (transaction) => transaction.selection && !transaction.isUserEvent('select.pointer'),
      )
    )
      this.gesture = null;
    if (
      update.startState.field(editorFreshness) !== update.state.field(editorFreshness) ||
      update.startState.facet(richRendering) !== update.state.facet(richRendering) ||
      (this.pending &&
        update.transactions.some((transaction) => transaction.selection) &&
        !update.state.selection.main.eq(this.pending))
    )
      this.pending = null;
    if (this.held !== update.state.field(heldProjection, false)?.session) this.held = null;
  }
  /** Keyboard, focus transfer and disposal invalidate the one pointer-only continuation. */
  clear(): void {
    this.pending = null;
    this.gesture = null;
    this.release();
  }
  /** Release one matching mounted-view reservation, with a disposed guard before native state access. */
  private release(): void {
    if (this.disposed || !this.held) return;
    const session = this.held;
    this.held = null;
    this.view.dispatch({ effects: releaseProjectionToken.of(session) });
  }
  /** Native view teardown releases the final transient target. */
  destroy(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.pending = null;
    this.held = null;
    this.gesture = null;
    this.unobserve();
  }
  /** Every passive callback belongs to this mounted view and is removed on teardown or partial admission failure. */
  private unobserve(): void {
    const failures: unknown[] = [];
    for (const name of ['mouseup', 'mousemove', 'mousedown', 'pointercancel'])
      try {
        this.document.removeEventListener(name, this.terminate, name === 'mousedown');
      } catch (cause) {
        failures.push(cause);
      }
    try {
      this.document.defaultView?.removeEventListener('blur', this.terminate);
    } catch (cause) {
      failures.push(cause);
    }
    if (failures.length === 1) throw failures[0];
    if (failures.length > 1)
      throw new AggregateError(failures, 'MOUSE_EXPOSURE_OBSERVER_CLEANUP_FAILED');
  }
  /** Capture native caret hits before focus scrolling; double presses use chip-start semantic intent for source selection and exposure. */
  style(event: MouseEvent): MouseSelectionStyle | null {
    if (this.disposed) return null;
    const view = this.view;
    const pending = this.pending;
    this.clear();
    const state = view.state;
    if (
      event.button !== 0 ||
      event.ctrlKey ||
      event.altKey ||
      event.metaKey ||
      view.compositionStarted
    )
      return null;
    const chip =
      typeof Element !== 'undefined' && event.target instanceof Element
        ? event.target.closest('.sp-lora-chip')
        : null;
    // The hit side distinguishes the end of one wrapped row from the start of the next.
    const hit: PointerHit | null =
      chip && event.detail === 2
        ? { pos: view.posAtDOM(chip, 0), assoc: 1 }
        : view.posAndSideAtCoords({ x: event.clientX, y: event.clientY });
    if (hit === null) return null;
    const position = hit.pos;
    if (this.disposed) return null;
    const freshness = state.field(editorFreshness),
      current = view.state.field(editorFreshness);
    if (
      freshness.source !== current.source ||
      freshness.context !== current.context ||
      state.facet(richRendering) !== view.state.facet(richRendering)
    )
      return null;
    let target: SelectionRange | null = null;
    let reveal: SourceRange | null = null;
    let sceneExposure: SourceRange | null = null;
    if (
      event.detail === 1 &&
      !event.shiftKey &&
      pending &&
      pending.from <= position &&
      position <= pending.to
    ) {
      const line = state.doc.lineAt(position);
      const word = promptWordRange(line.text, position - line.from);
      if (word.from !== word.to)
        target = EditorSelection.range(line.from + word.from, line.from + word.to);
    } else if (event.detail === 2) {
      const prepared = state.field(semanticStructures).native;
      if (!prepared) return null;
      const targets: Record<'segment' | 'scene' | 'token' | 'body', SelectionRange | null> = {
        segment: null,
        scene: null,
        token: null,
        body: null,
      };
      let width = Infinity;
      prepared.pointerTargets.between(position, position, (from, to, value) => {
        if (value.kind === 'segment') targets.segment = EditorSelection.range(from, to);
        else if (value.kind === 'scene' && state.facet(richRendering)) {
          targets.scene = EditorSelection.range(from, to);
          sceneExposure = { from, to };
        } else if (
          (value.kind === 'lora' || value.kind === 'wildcard') &&
          state.facet(richRendering) &&
          featureEnabled(
            state.facet(featureProfile),
            value.kind === 'lora' ? 'lora_syntax' : 'wildcard_syntax',
          )
        ) {
          targets.token = EditorSelection.range(from, to);
        } else if (
          value.kind === 'emphasis' &&
          state.facet(richRendering) &&
          featureEnabled(state.facet(featureProfile), 'emphasis') &&
          to - from < width
        ) {
          targets.body = EditorSelection.range(from + value.contentFrom, from + value.contentTo);
          reveal = { from, to };
          width = to - from;
        }
      });
      target = targets.scene ?? targets.token ?? targets.body ?? targets.segment;
      if (targets.scene) reveal = sceneExposure;
      else if (targets.token)
        reveal = {
          from: targets.token.from,
          to: targets.token.to,
        };
      this.pending = target;
    }
    let pointerAnchor: PointerHit | null = null;
    if (!target && event.detail === 1) {
      // A click on a regional separator row places the caret in that region's content.
      const content = event.shiftKey ? hit.pos : regionContentCaret(state, hit.pos, true);
      pointerAnchor = content === hit.pos ? hit : { pos: content, assoc: 1 };
      target = EditorSelection.cursor(pointerAnchor.pos, pointerAnchor.assoc);
    }
    if (!target) return null;
    if (reveal) {
      const projection = currentInputProjection(state);
      if (projection) {
        const session = Symbol();
        this.held = session;
        try {
          view.dispatch({
            effects: holdProjectionToken.of({
              session,
              freshness: state.field(editorFreshness),
              range: reveal,
              prepared: projection,
            }),
          });
        } catch (error) {
          this.held = null;
          throw error;
        }
        if (this.disposed || this.held !== session) return null;
      }
    }
    let range = target,
      original = state.selection;
    const gesture = Symbol();
    this.gesture = gesture;
    let lastSelection = original;
    return {
      /** CodeMirror applies this source range and owns the complete native pointer gesture. */
      get: (currentEvent, extend, multiple) => {
        if (this.disposed || this.gesture !== gesture) return lastSelection;
        let current: SelectionRange;
        if (currentEvent === event)
          current =
            pointerAnchor === null
              ? range
              : EditorSelection.cursor(pointerAnchor.pos, pointerAnchor.assoc);
        else {
          const { pos: head, assoc } = view.posAndSideAtCoords(
            { x: currentEvent.clientX, y: currentEvent.clientY },
            false,
          );
          if (this.disposed || this.gesture !== gesture) return lastSelection;
          if (pointerAnchor !== null)
            current =
              head === pointerAnchor.pos
                ? EditorSelection.cursor(head, assoc)
                : EditorSelection.range(pointerAnchor.pos, head, undefined, undefined, assoc);
          else {
            const line = view.state.doc.lineAt(head),
              word = promptWordRange(line.text, head - line.from);
            current =
              head < range.from
                ? EditorSelection.range(range.to, line.from + word.from)
                : head > range.to
                  ? EditorSelection.range(range.from, line.from + word.to)
                  : range;
          }
        }
        lastSelection = multiple
          ? EditorSelection.create([...original.ranges, current], original.ranges.length)
          : extend
            ? EditorSelection.single(original.main.anchor, current.head)
            : EditorSelection.create([current]);
        return lastSelection;
      },
      /** Native document updates map the active gesture without another mutable document owner. */
      update: (update) => {
        if (this.disposed) return;
        if (this.gesture !== gesture) {
          lastSelection = update.state.selection;
          return;
        }
        if (update.docChanged) {
          range = range.map(update.changes);
          original = original.map(update.changes);
          lastSelection = lastSelection.map(update.changes);
          if (pointerAnchor !== null)
            pointerAnchor = {
              pos: update.changes.mapPos(pointerAnchor.pos),
              assoc: pointerAnchor.assoc,
            };
        }
      },
    };
  }
}
/** Native extension preserves captured initial pointer placement and prepared segment/word intent; CodeMirror owns the gesture lifetime. */
export const promptMouseSelection = (() => {
  const plugin = ViewPlugin.fromClass(PromptMouseSelection, {
    eventObservers: {
      /** A key gesture ends the segment-to-word continuation before native authoring/navigation. */
      keydown() {
        this.clear();
      },
      /** Leaving the editor ends the pointer-only continuation. */
      blur() {
        this.clear();
      },
      /** Composition retires token geometry before native composing input begins. */
      compositionstart() {
        this.clear();
      },
    },
  });
  return [
    plugin,
    EditorView.mouseSelectionStyle.of((view, event) => view.plugin(plugin)?.style(event) ?? null),
  ];
})();
