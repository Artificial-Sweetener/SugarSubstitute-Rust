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
/** One finite native numeric widget interaction delegates committed source, caret and history to the shared queue. */
import { captureFocusHandoff } from './focus-handoff.js';
import type { FocusHandoff } from './focus-handoff-contracts.js';
import { Prec, Transaction } from '@codemirror/state';
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import { parseWeight } from '../../core/src/prompt/weights.js';
import { editorFreshness } from './freshness.js';
import { richRendering } from './presentation.js';
import type { SpatialWeightRequest } from './weight-command.js';
import { weightSession } from './weight-session.js';
import { beginExactWeight, cancelTargetWeight, queueExactWeight } from './weight-interactions.js';
import { weightEditingAllowed, weightLabelSelector } from './weight-target.js';
import { labelTarget } from './weight-control-geometry.js';
import { ExactWeightInput, validWeightDraft } from './exact-weight-input.js';
import { exactWeightDraft, setExactWeightDraft } from './exact-weight-draft.js';
import { weightInputFactory, type WeightInputMount } from './weight-input-mount.js';
/** Native input owns the numeric value; this view owns source-bound intent and finite mount/focus callbacks. */
class ExactWeights {
  private input: ExactWeightInput | null = null;
  private target: Omit<Extract<SpatialWeightRequest, { owner: 'exact' }>, 'completion'> | null =
    null;
  private key: symbol | null = null;
  private handoff: FocusHandoff | null = null;
  private initial = '';
  private disposed = false;
  private readonly view: EditorView;
  /** Bind numeric intent to one supported native editor lifetime. */
  constructor(view: EditorView) {
    this.view = view;
  }
  /** Source/context/permissions or another native intent retire the exact input. */
  update(update: ViewUpdate): void {
    if (
      this.target &&
      (update.state.field(editorFreshness) !== this.target.freshness ||
        !this.allowed() ||
        update.state.field(weightSession)?.owner !== 'exact')
    ) {
      const key = this.key;
      this.close(false);
      queueMicrotask(() => {
        if (!this.disposed && this.view.state.field(exactWeightDraft)?.key === key) this.clear();
      });
    }
  }
  /** Numeric presses preserve prompt selection; only double presses or transient neutral values activate authoring. */
  press(event: MouseEvent): boolean {
    if (event.target instanceof Element && event.target.closest('.sp-exact-weight-input'))
      return false;
    if (event.button !== 0 || event.ctrlKey || event.altKey || event.metaKey || !this.allowed())
      return false;
    const label =
      event.target instanceof Element
        ? event.target.closest<HTMLElement>(weightLabelSelector)
        : null;
    if (!label) return false;
    event.preventDefault();
    if (event.detail < 2 && label.dataset.weightNeutral !== 'true') return true;
    const target = labelTarget(this.view, label);
    if (!target) return true;
    this.close(true);
    this.initial = label.textContent ?? '';
    this.key = Symbol('native weight draft');
    this.target = {
      owner: 'exact',
      freshness: this.view.state.field(editorFreshness),
      ...target,
      neutral: label.dataset.weightNeutral === 'true',
    };
    this.handoff = captureFocusHandoff(this.view, 'input');
    beginExactWeight(this.view, { ...this.target, boundary: 'end' });
    window.addEventListener('pointerdown', this.outside, true);
    this.view.dispatch({
      effects: setExactWeightDraft.of({
        ...this.target,
        key: this.key,
        phase: 'editing',
      }),
      annotations: Transaction.addToHistory.of(false),
    });
    return true;
  }
  /** Widget creation delegates to the current finite owner; stale keys cannot attach a replacement input. */
  mount(key: symbol, cell: HTMLElement, value: string): WeightInputMount {
    if (this.disposed || key !== this.key || !this.target || !this.allowed())
      throw new Error('Stale native weight input mount');
    const input = new ExactWeightInput(cell, this.target.kind, value, (completion, cancel) =>
      this.finish(completion, cancel),
    );
    this.input = input;
    queueMicrotask(() => {
      if (!this.disposed && this.input === input && cell.isConnected && this.allowed())
        input.focus();
    });
    return {
      /** Widget removal releases its own input and retires only that still-current mount. */
      release: () => {
        input.destroy();
        queueMicrotask(() => {
          if (!this.disposed && this.input === input) this.cancel();
        });
      },
    };
  }
  /** Unfinished native prompt composition and current host permissions exclude numeric mutation. */
  private allowed(): boolean {
    return (
      !this.disposed &&
      this.view.state.facet(richRendering) &&
      weightEditingAllowed(this.view.state, this.target?.kind) &&
      !this.view.compositionStarted &&
      !this.view.composing
    );
  }
  /** Finalize before a prompt pointer gesture receives its next native source selection. */
  private readonly outside = (event: PointerEvent): void => {
    if (this.input && event.target !== this.input.element) this.finish('outside', false);
  };
  /** Capture completion before reentrancy; accepted drafts retain their native cell until the queue publishes source. */
  private finish(completion: 'keyboard' | 'outside' | 'space', cancel: boolean): void {
    const target = this.target,
      input = this.input;
    if (!target || !input) return;
    const text = input.element.value;
    const value =
      !cancel && (validWeightDraft(text) || text === this.initial)
        ? parseWeight(text, false) === null
          ? null
          : text
        : null;
    const handoff = this.handoff;
    if (handoff === null) throw new Error('Native weight input handoff missing');
    this.handoff = null;
    const key = this.key;
    if (key === null) throw new Error('Native weight input key missing');
    this.close(cancel || value === null);
    if (
      this.disposed ||
      this.target !== null ||
      this.input !== null ||
      (this.view.state.field(exactWeightDraft) &&
        this.view.state.field(exactWeightDraft)?.key !== key)
    ) {
      handoff.cancel();
      return;
    }
    if (cancel || value === null) {
      cancelTargetWeight(this.view, 'exact');
      if (completion !== 'outside') void handoff.restore('preserve-viewport');
      else handoff.cancel();
      return;
    }
    if (!queueExactWeight(this.view, value, { ...target, completion }, handoff, key)) {
      handoff.cancel();
      cancelTargetWeight(this.view, 'exact');
      if (this.view.state.field(exactWeightDraft)?.key === key) this.clear();
    }
  }
  /** Widget eviction cancels current intent without committing an offscreen value. */
  private cancel(): void {
    this.close(true);
    cancelTargetWeight(this.view, 'exact');
  }
  /** Remove transient rendering through supported native state rather than editing a managed widget parent. */
  private clear(): void {
    if (this.view.state.field(exactWeightDraft))
      this.view.dispatch({
        effects: setExactWeightDraft.of(null),
        annotations: Transaction.addToHistory.of(false),
      });
  }
  /** Release local input/listeners first; native state and widgets own rendered subtree replacement. */
  private close(clear: boolean): void {
    const key = this.key,
      handoff = this.handoff;
    this.handoff = null;
    this.key = null;
    this.target = null;
    this.initial = '';
    const input = this.input;
    this.input = null;
    window.removeEventListener('pointerdown', this.outside, true);
    input?.destroy();
    handoff?.cancel();
    if (
      clear &&
      !this.disposed &&
      this.target === null &&
      this.input === null &&
      this.view.state.field(exactWeightDraft)?.key === key
    )
      this.clear();
  }
  /** Teardown prevents queued mount/focus/disposal callbacks from publishing afterward. */
  destroy(): void {
    this.disposed = true;
    this.close(false);
  }
}
const exactWeightPlugin = ViewPlugin.fromClass(ExactWeights, {
  eventHandlers: {
    /** Prioritize source-backed numeric activation over ordinary prompt segment selection. */
    mousedown(event) {
      return this.press(event);
    },
    /** Double-click inside the native input belongs to its own text selection. */
    dblclick(event) {
      return (
        event.target instanceof Element &&
        !event.target.closest('.sp-exact-weight-input') &&
        !!event.target.closest(weightLabelSelector)
      );
    },
  },
});
/** One supported extension installs native input ownership and typed widget mount delegation together. */
export const exactWeightInteractions = [
  exactWeightDraft,
  Prec.high(exactWeightPlugin),
  weightInputFactory.of((view, key, cell, value) => {
    const owner = view.plugin(exactWeightPlugin);
    if (!owner) throw new Error('Native weight interaction missing');
    return owner.mount(key, cell, value);
  }),
];
