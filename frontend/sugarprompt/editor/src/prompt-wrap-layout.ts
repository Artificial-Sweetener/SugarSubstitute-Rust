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
/** Measure optional nowrap units through CodeMirror's layout lifecycle without changing prompt source. */
import { RangeSet, StateEffect, StateField, type Extension, type Text } from '@codemirror/state';
import {
  Decoration,
  EditorView,
  ViewPlugin,
  type DecorationSet,
  type ViewUpdate,
} from '@codemirror/view';
import { editorFreshness } from './freshness.js';
import { richRendering } from './presentation.js';
import { richProjection } from './rich-projection.js';
import { snapshotPresentation } from './native-snapshot-policy.js';
import { measurePromptWrapGroup } from './prompt-wrap-measurement.js';
import {
  fittingPromptWrapGroups,
  preparePromptWrapGroups,
  type PromptWrapGroup,
} from './prompt-wrap-groups.js';

interface WrapPublication {
  readonly document: Text;
  readonly decorations: DecorationSet;
}
const publishWrap = StateEffect.define<WrapPublication>();
/** Retain only native marks; the layout owner alone chooses their ranges and measured widths. */
const wrapMarks = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(value, transaction) {
    if (!transaction.state.facet(richRendering)) return Decoration.none;
    if (transaction.docChanged) {
      transaction.changes.iterChangedRanges((from, to) => {
        value = value.update({
          filterFrom: from,
          filterTo: to,
          filter: (start, end) => end < from || start > to,
        });
      });
      value = value.map(transaction.changes);
    }
    for (const effect of transaction.effects)
      if (effect.is(publishWrap) && effect.value.document === transaction.state.doc)
        value = effect.value.decorations;
    return value;
  },
  provide: (field) => EditorView.outerDecorations.from(field),
});

interface WrapMeasurement {
  readonly groups: readonly PromptWrapGroup[];
  readonly font: string;
  readonly scale: number;
  readonly width: number;
  readonly widths: ReadonlyMap<PromptWrapGroup, number>;
}

/** Match unchanged measured units independently of a regenerated semantic token's identity. */
function groupKey(group: PromptWrapGroup): string {
  return `${group.kind}:${String(group.from)}:${String(group.to)}`;
}

/** Admit full-line-fitting groups, including true rendered widget widths, at each native layout boundary. */
class PromptWrapLayout {
  private readonly view: EditorView;
  readonly #measurementRange: Range;
  #groups: readonly PromptWrapGroup[] = [];
  #widths = new Map<PromptWrapGroup, number>();
  #font = '';
  #scale = 0;
  #paint = '';
  #projection: DecorationSet | null = null;
  #widthProjection: DecorationSet | null = null;
  #sourceGroups: readonly PromptWrapGroup[] = [];
  #indices = new Map<PromptWrapGroup, number>();
  #document: Text | null = null;
  #disposed = false;

  /** Prepare after native mount, when the authoritative rich projection can become available. */
  constructor(view: EditorView) {
    this.view = view;
    this.#measurementRange = view.dom.ownerDocument.createRange();
    view.dom.ownerDocument.fonts?.addEventListener('loadingdone', this.#fontsLoaded);
    this.#prepare();
  }

  /** Newly loaded faces change glyph widths without changing the computed font description. */
  readonly #fontsLoaded = (): void => {
    if (this.#disposed) return;
    this.#widths = new Map();
    this.#measure();
  };

  /** Reuse prepared semantic boundaries and let native geometry observation coalesce measurement. */
  update(update: ViewUpdate): void {
    if (update.docChanged) {
      const widths = new Map<PromptWrapGroup, number>();
      for (const [group, width] of this.#widths) {
        if (update.changes.touchesRange(group.from, group.to)) continue;
        widths.set(
          {
            ...group,
            from: update.changes.mapPos(group.from, 1),
            to: update.changes.mapPos(group.to, -1),
          },
          width,
        );
      }
      this.#widths = widths;
      this.#widthProjection = this.#widthProjection?.map(update.changes) ?? null;
    }
    const changed = this.#prepare();
    if (changed || update.geometryChanged || update.viewportChanged) this.#measure();
  }

  /** Retire queued measurement without leaving another editable document or observer. */
  destroy(): void {
    this.#disposed = true;
    this.view.dom.ownerDocument.fonts?.removeEventListener('loadingdone', this.#fontsLoaded);
  }

  /** Rebuild grouping only after a current source projection, including explicit raw-mode retirement. */
  #prepare(): boolean {
    const state = this.view.state,
      projection = state.field(richProjection);
    const current =
      state.facet(richRendering) &&
      projection.sourceIdentity === state.field(editorFreshness).source;
    if (state.facet(richRendering) && !current) {
      this.#document = null;
      this.#projection = null;
      return false;
    }
    const decorations = current ? projection.decorations : null;
    if (this.#document === state.doc && this.#projection === decorations) return false;
    this.#document = state.doc;
    this.#projection = decorations;
    const groups = decorations ? preparePromptWrapGroups(state.doc.toString(), decorations) : [];
    const previous = new Map([...this.#widths].map(([group, width]) => [groupKey(group), width]));
    const widths = new Map<PromptWrapGroup, number>();
    if (decorations && this.#widthProjection)
      for (const group of groups) {
        const width = previous.get(groupKey(group));
        if (
          width !== undefined &&
          RangeSet.eq([this.#widthProjection], [decorations], group.from, group.to)
        )
          widths.set(group, width);
      }
    this.#groups = groups;
    this.#sourceGroups = [...groups].sort((left, right) => left.from - right.from);
    this.#indices = new Map(this.#groups.map((group, index) => [group, index]));
    this.#widths = widths;
    this.#widthProjection = decorations;
    this.#paint = '';
    this.#measure();
    return true;
  }

  /** Measure complete native viewport units before admitting nowrap marks; keep unaffected measured widths through source edits. */
  #measure(): void {
    if (!this.#document) return;
    this.view.requestMeasure<WrapMeasurement | null>({
      key: this,
      read: (view) => {
        // A source edit since the request leaves these groups stale until the next projection.
        if (this.#disposed || view.state.doc !== this.#document) return null;
        const content = view.contentDOM,
          style = getComputedStyle(content),
          rect = content.getBoundingClientRect();
        const scale = content.offsetWidth ? rect.width / content.offsetWidth : 1;
        const width =
          rect.width / scale -
          Number.parseFloat(style.paddingLeft) -
          Number.parseFloat(style.paddingRight);
        const font = [style.font, style.letterSpacing, style.wordSpacing].join('|');
        const widths = new Map(this.#font === font && this.#scale === scale ? this.#widths : []);
        for (const visible of view.visibleRanges) {
          const visibleFrom = visible.from;
          const visibleTo = visible.to;
          let from = 0,
            to = this.#sourceGroups.length;
          while (from < to) {
            const middle = (from + to) >>> 1;
            if ((this.#sourceGroups[middle]?.from ?? Infinity) < visibleFrom) from = middle + 1;
            else to = middle;
          }
          for (let index = from; index < this.#sourceGroups.length; index++) {
            const group = this.#sourceGroups[index];
            if (!group || group.from >= visibleTo) break;
            // A group's natural width survives edits elsewhere; only unmeasured groups need layout reads.
            if (group.to <= visibleTo && !widths.has(group))
              widths.set(group, measurePromptWrapGroup(view, group, scale, this.#measurementRange));
          }
        }
        return { groups: this.#groups, font, scale, width, widths };
      },
      write: (measurement, view) => {
        if (
          !measurement ||
          this.#disposed ||
          measurement.groups !== this.#groups ||
          view.state.doc !== this.#document
        )
          return;
        this.#font = measurement.font;
        this.#scale = measurement.scale;
        this.#widths = new Map(measurement.widths);
        const selected = fittingPromptWrapGroups(this.#groups, this.#widths, measurement.width);
        const paint = selected.map((group) => String(this.#indices.get(group))).join(',');
        if (paint === this.#paint && view.state.field(wrapMarks).size === selected.length) return;
        this.#paint = paint;
        const decorations = Decoration.set(
          selected.map((group) =>
            Decoration.mark({
              class: 'sp-prompt-keep',
              inclusive: true,
              attributes: { 'data-sp-prompt-keep': String(this.#indices.get(group)) },
            }).range(group.from, group.to),
          ),
          true,
        );
        const groups = this.#groups,
          document = view.state.doc;
        // CodeMirror forbids dispatch while its measure WRITE is active.
        queueMicrotask(() => {
          if (
            this.#disposed ||
            this.#groups !== groups ||
            view.state.doc !== document ||
            this.#paint !== paint
          )
            return;
          view.dispatch({
            effects: publishWrap.of({ document, decorations }),
            annotations: snapshotPresentation.of(true),
          });
          this.#measure();
        });
      },
    });
  }
}

/** Keep source-native caret, selection and history while applying conditional visual grouping. */
export const promptWrapLayout: Extension = [
  wrapMarks,
  ViewPlugin.fromClass(PromptWrapLayout),
  EditorView.theme({ '.sp-prompt-keep': { whiteSpace: 'nowrap' } }),
];
