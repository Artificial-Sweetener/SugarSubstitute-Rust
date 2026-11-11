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
/** One finite tooltip lifetime animates only its prepared active overflowing title. */
import type { EditorView } from '@codemirror/view';
import { titleMarqueeState } from './title-marquee.js';

/** Measured title nodes belong to the tooltip, never to CodeMirror's editable content. */
interface MeasuredTitle {
  readonly tile: HTMLElement;
  readonly label: HTMLElement;
  readonly text: HTMLElement;
  readonly overflow: number;
  readonly fadeWidth: number;
}

/** Hover overrides observed native selection; one animation frame consumes scalar prepared geometry. */
export class CompletionTitleMarquee {
  private readonly view: EditorView;
  private readonly root: HTMLElement;
  private readonly selected: () => HTMLElement | null;
  private readonly window: Window;
  private hovered: HTMLElement | null = null;
  private lastSelected: HTMLElement | null = null;
  private active: MeasuredTitle | null = null;
  private started = 0;
  private frame: number | null = null;
  private queued = false;
  private restart = false;
  private disposed = false;
  /** Observe only owned tile hover and the document's finite visibility lifetime. */
  constructor(view: EditorView, root: HTMLElement, selected: () => HTMLElement | null) {
    this.view = view;
    this.root = root;
    this.selected = selected;
    const window = root.ownerDocument.defaultView;
    if (!window) throw new Error('Completion title window unavailable');
    this.window = window;
    root.addEventListener('mousemove', this.moved);
    root.addEventListener('mouseleave', this.left);
    root.ownerDocument.addEventListener('visibilitychange', this.visibility);
  }
  /** Selection, candidate membership and resize publication request bounded current-title measurement. */
  refresh(restart = false): void {
    this.restart ||= restart;
    if (this.disposed || this.queued) return;
    this.queued = true;
    this.view.requestMeasure({
      key: this,
      /** Read one current title after native wall placement; no catalog walk or font measurement occurs in input. */
      read: () => {
        const selected = this.selected();
        const tile = this.hovered && this.root.contains(this.hovered) ? this.hovered : selected;
        const label = tile?.querySelector<HTMLElement>('.cm-completionLabel'),
          text = label?.querySelector<HTMLElement>('.sp-completion-title-text');
        return {
          selected,
          measured:
            tile && label && text && tile.style.visibility !== 'hidden'
              ? {
                  tile,
                  label,
                  text,
                  overflow:
                    (text.getBoundingClientRect().width * label.clientWidth) /
                      label.getBoundingClientRect().width -
                    label.clientWidth,
                  fadeWidth: Math.min(18, Math.max(1, Math.floor(label.clientWidth / 2))),
                }
              : null,
        };
      },
      /** Superseded nodes cannot revive after candidate replacement or tooltip teardown. */
      write: ({ selected, measured }) => {
        this.queued = false;
        if (this.disposed) return;
        if (this.hovered && !this.root.contains(this.hovered)) this.hovered = null;
        if (selected !== this.selected() || (measured && !this.root.contains(measured.tile))) {
          this.refresh();
          return;
        }
        const next = measured && measured.overflow > 0 ? measured : null;
        if (
          this.restart ||
          selected !== this.lastSelected ||
          next?.tile !== this.active?.tile ||
          next?.overflow !== this.active?.overflow
        ) {
          this.reset();
          this.active = next;
          this.started = this.window.performance.now();
        }
        this.lastSelected = selected;
        this.restart = false;
        this.animate();
      },
    });
  }
  /** Cancel frames and listeners, releasing all retained title nodes; pending native measurements become inert. */
  destroy(): void {
    this.disposed = true;
    this.reset();
    this.hovered = null;
    this.lastSelected = null;
    this.root.removeEventListener('mousemove', this.moved);
    this.root.removeEventListener('mouseleave', this.left);
    this.root.ownerDocument.removeEventListener('visibilitychange', this.visibility);
  }
  /** Pointer hover is a visual observation, never a selected-row or caret mutation. */
  private readonly moved = (event: MouseEvent): void => {
    const tile =
      event.target instanceof Element
        ? event.target.closest<HTMLElement>('.sp-completion-tile')
        : null;
    const next = tile && this.root.contains(tile) ? tile : null;
    if (next !== this.hovered) {
      this.hovered = next;
      this.refresh();
    }
  };
  /** Leaving the viewport returns title animation to the native selected option. */
  private readonly left = (): void => {
    this.hovered = null;
    this.refresh();
  };
  /** Hidden documents retain no animation work; returning resumes a readable start hold. */
  private readonly visibility = (): void => {
    this.reset();
    this.refresh();
  };
  /** Static elision resumes whenever animation ownership moves or retires. */
  private reset(): void {
    if (this.frame !== null) this.window.cancelAnimationFrame(this.frame);
    this.frame = null;
    if (this.active) {
      delete this.active.label.dataset.marquee;
      this.active.label.style.maskImage = '';
      this.active.text.style.transform = '';
    }
    this.active = null;
  }
  /** Paint only prepared scalar phases; frame callbacks perform no DOM geometry reads. */
  private animate(): void {
    if (this.disposed || !this.active || this.frame !== null || this.root.ownerDocument.hidden)
      return;
    const active = this.active;
    if (!this.root.contains(active.tile)) {
      this.reset();
      return;
    }
    const state = titleMarqueeState(
      Math.round(this.window.performance.now() - this.started),
      active.overflow,
    );
    active.label.dataset.marquee = state.phase;
    active.text.style.transform = state.phase === 'scroll' ? `translateX(${-state.offset}px)` : '';
    active.label.style.maskImage =
      state.phase === 'scroll'
        ? `linear-gradient(to right, ${state.showLeftFade ? `transparent, black ${active.fadeWidth}px` : 'black'}, ${state.showRightFade ? `black calc(100% - ${active.fadeWidth}px), transparent` : 'black'})`
        : '';
    this.frame = this.window.requestAnimationFrame(() => {
      this.frame = null;
      this.animate();
    });
  }
}
