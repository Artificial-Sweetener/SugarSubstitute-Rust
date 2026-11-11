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
/** One native view owns passive post-adjustment preview measurement, expiry and supersession. */
import { requireValue } from '../../core/src/required.js';
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import { editorFreshness } from './freshness.js';
import { richRendering } from './presentation.js';
import { weightEditingAllowed } from './weight-target.js';
import { weightSession, type WeightSession } from './weight-session.js';
import { weightPreviewText } from './weight-preview-value.js';
import {
  WeightPreviewView,
  weightPreviewPlacement,
  type WeightPreviewTypography,
} from './weight-preview-view.js';
import type { WheelPoint } from './wheel-intent.js';
import type { WeightRect } from './weight-control-geometry.js';
/** Bounded physical inputs describe one painted portal without retaining live DOM rectangles or editable state. */
interface WeightPreviewInputs {
  readonly font: WeightPreviewTypography;
  readonly viewport: WeightRect;
}
/** The finite preview observes accepted source-current pointer/wheel sessions; it has no editing or command authority. */
export class WeightPreview {
  private readonly surface: WeightPreviewView;
  private readonly view: EditorView;
  /** The one accepted pending or painted session observes native authority without owning source or selection. */
  private session: WeightSession | null = null;
  private timer: number | null = null;
  private painted: WeightPreviewInputs | null = null;
  private geometryPending = false;
  private readonly geometryKey = {};
  private generation = 0;
  private disposed = false;
  /** Application focus loss retires passive feedback immediately. */
  private readonly blur = (): void => this.clear();
  /** Bind one portal and one finite deadline to the mounted native document. */
  constructor(view: EditorView) {
    this.view = view;
    this.surface = new WeightPreviewView(view.dom.ownerDocument);
    requireValue(view.dom.ownerDocument.defaultView, 'weight-preview').addEventListener(
      'blur',
      this.blur,
    );
  }
  /** Source/context, native selection, presentation, permission and geometry changes retire old feedback. */
  update(update: ViewUpdate): void {
    if (this.disposed) return;
    if (
      update.startState.field(editorFreshness) !== update.state.field(editorFreshness) ||
      update.state.field(weightSession) !== this.session ||
      update.selectionSet ||
      !update.state.facet(richRendering) ||
      !weightEditingAllowed(update.state, this.session?.kind)
    )
      this.clear();
    else if (this.timer !== null && (update.geometryChanged || update.viewportChanged))
      this.validateGeometry();
  }
  /** Record accepted spatial intent before yielding; same-session derived updates preserve it and retirement invalidates queued native observation. */
  enqueue(session: WeightSession, point: WheelPoint): void {
    if (this.disposed) return;
    this.clear();
    this.session = session;
    const generation = this.generation,
      capturedPoint = { x: point.x, y: point.y };
    queueMicrotask(() => {
      if (this.disposed || generation !== this.generation) return;
      if (this.view.state.field(weightSession, false) === session)
        this.show(session, capturedPoint);
      else this.clear();
    });
  }
  /** Accepted spatial publication resolves one exact prepared token and schedules supported native geometry phases. */
  show(session: WeightSession, point: WheelPoint): void {
    if (this.disposed) return;
    this.clear();
    if (
      (session.owner !== 'pointer' && session.owner !== 'wheel') ||
      !this.view.state.facet(richRendering) ||
      !weightEditingAllowed(this.view.state, session.kind) ||
      this.view.composing ||
      this.view.compositionStarted
    )
      return;
    const text = weightPreviewText(this.view.state, session);
    if (text === null) return;
    this.session = session;
    const generation = this.generation;
    const current = (): boolean =>
      !this.disposed &&
      generation === this.generation &&
      this.view.state.field(weightSession) === session &&
      this.view.state.facet(richRendering) &&
      weightEditingAllowed(this.view.state, session.kind) &&
      !this.view.composing &&
      !this.view.compositionStarted;
    this.schedule({
      key: this,
      read: () =>
        this.observe(() => {
          if (!current()) return null;
          return this.inputs();
        }),
      write: (prepared) => {
        if (!current() || !prepared) return;
        this.observe(() => this.surface.prepare(text, prepared.font, prepared.viewport));
        this.schedule({
          key: this,
          read: () =>
            this.observe(() => {
              if (!current()) return null;
              const size = this.surface.measure(),
                viewport = this.viewport();
              return {
                viewport,
                rect: weightPreviewPlacement(size, point, viewport),
              };
            }),
          write: (placed) => {
            if (!current()) return;
            if (!placed?.rect) {
              this.clear();
              return;
            }
            const rect = placed.rect;
            this.observe(() => {
              this.surface.show(rect);
              this.painted = { font: prepared.font, viewport: placed.viewport };
              this.timer = requireValue(
                this.view.dom.ownerDocument.defaultView,
                'weight-preview',
              ).setTimeout(() => {
                if (!current()) return;
                this.timer = null;
                this.clear();
              }, 240);
            });
          },
        });
      },
    });
  }
  /** Copy current viewport pixels during a supported read phase; retained inputs cannot alias later native rectangles. */
  private viewport(): WeightRect {
    const { left, top, width, height } = this.view.scrollDOM.getBoundingClientRect();
    return { left, top, width, height };
  }
  /** Sample only the physical typography and viewport inputs consumed by passive preparation and placement. */
  private inputs(): WeightPreviewInputs {
    const style = getComputedStyle(this.view.contentDOM);
    return {
      font: {
        fontFamily: style.fontFamily,
        fontSize: style.fontSize,
        color: style.color,
        colorScheme: style.colorScheme,
        scale: this.view.scaleY,
      },
      viewport: this.viewport(),
    };
  }
  /** Generic native layout notifications validate one painted snapshot without rewriting the portal or renewing its original expiry. */
  private validateGeometry(): void {
    const painted = this.painted,
      session = this.session;
    if (!painted || !session || this.geometryPending) return;
    this.geometryPending = true;
    const generation = this.generation;
    const current = (): boolean =>
      !this.disposed &&
      generation === this.generation &&
      this.geometryPending &&
      this.painted === painted &&
      this.timer !== null &&
      this.view.state.field(weightSession) === session &&
      this.view.state.facet(richRendering) &&
      weightEditingAllowed(this.view.state, session.kind) &&
      !this.view.composing &&
      !this.view.compositionStarted;
    this.schedule({
      key: this.geometryKey,
      read: () => this.observe(() => (current() ? this.inputs() : null)),
      write: (inputs) => {
        if (!current()) return;
        this.geometryPending = false;
        if (
          !inputs ||
          inputs.font.fontFamily !== painted.font.fontFamily ||
          inputs.font.fontSize !== painted.font.fontSize ||
          inputs.font.color !== painted.font.color ||
          inputs.font.colorScheme !== painted.font.colorScheme ||
          inputs.font.scale !== painted.font.scale ||
          inputs.viewport.left !== painted.viewport.left ||
          inputs.viewport.top !== painted.viewport.top ||
          inputs.viewport.width !== painted.viewport.width ||
          inputs.viewport.height !== painted.viewport.height
        )
          this.clear();
      },
    });
  }
  /** Supported native scheduling failures retire the owned preview before preserving their cause. */
  private schedule<T>(request: {
    read(view: EditorView): T;
    write?(value: T, view: EditorView): void;
    key?: unknown;
  }): void {
    this.observe(() => this.view.requestMeasure(request));
  }
  /** Unexpected native transport failures retire retained feedback and preserve the original observable cause. */
  private observe<T>(action: () => T): T {
    try {
      return action();
    } catch (cause) {
      const failures: unknown[] = [cause];
      this.retire(failures);
      this.report(failures);
      throw cause;
    }
  }
  /** Snapshot and retire authority before attempting every independent external release. */
  private retire(failures: unknown[]): void {
    this.generation++;
    const timer = this.timer;
    this.timer = null;
    this.session = null;
    this.painted = null;
    this.geometryPending = false;
    if (timer !== null)
      this.release(failures, () =>
        requireValue(this.view.dom.ownerDocument.defaultView, 'weight-preview').clearTimeout(timer),
      );
    this.release(failures, () => this.surface.hide());
  }
  /** A failed transport release cannot skip the next owned resource or replace its cause. */
  private release(failures: unknown[], action: () => void): void {
    try {
      action();
    } catch (cause) {
      failures.push(cause);
    }
  }
  /** Single thrown values retain identity, including undefined; multiple causes retain release order. */
  private report(failures: readonly unknown[]): void {
    if (failures.length === 1) throw failures[0];
    if (failures.length > 1) throw new AggregateError(failures, 'Weight preview cleanup failed');
  }
  /** Supersession cancels the one deadline and makes every scheduled read/write inert. */
  clear(): void {
    if (this.disposed) return;
    const failures: unknown[] = [];
    this.retire(failures);
    this.report(failures);
  }
  /** Teardown removes the window listener, timer and passive native portal. */
  destroy(): void {
    if (this.disposed) return;
    this.disposed = true;
    const failures: unknown[] = [];
    this.retire(failures);
    this.release(failures, () =>
      requireValue(this.view.dom.ownerDocument.defaultView, 'weight-preview').removeEventListener(
        'blur',
        this.blur,
      ),
    );
    this.release(failures, () => this.surface.destroy());
    this.report(failures);
  }
}
/** Install the passive owner beside the canonical weight command lifetime. */
export const weightPreview = ViewPlugin.fromClass(WeightPreview, {
  eventObservers: {
    /** Native authoring or another weight intent retires the previous short-lived value. */
    keydown() {
      this.clear();
    },
    /** Unfinished preedit cannot retain post-adjustment feedback. */
    compositionstart() {
      this.clear();
    },
    /** Pointer departure retires this editor's transient feedback. */
    mouseleave() {
      this.clear();
    },
    /** Native focus transfer retires the source-derived value. */
    blur() {
      this.clear();
    },
  },
});
/** Existing spatial owners alone request feedback after their accepted current session publication. */
export function showWeightPreview(
  view: EditorView,
  session: WeightSession,
  point: WheelPoint,
): void {
  view.plugin(weightPreview)?.enqueue(session, point);
}
