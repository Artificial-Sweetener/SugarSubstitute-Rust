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
/** Native Unicode segmentation prepares one derived logical-document line for source motion and deletion. */
import type { Text } from '@codemirror/state';
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import type { SourceRange } from '../../core/src/offsets.js';

const segmenter = new Intl.Segmenter('und', { granularity: 'grapheme' });

/** One native segmentation result uses logical-document UTF-16 line boundaries; it never owns editable source. */
interface GraphemeLine {
  readonly document: Text;
  readonly from: number;
  readonly to: number;
  readonly segments: Intl.Segments;
}

/** Memoize at most one actual native line result; unchanged source and caret movement reuse its segmentation. */
export class GraphemeLineCache {
  private current: GraphemeLine | null = null;

  /** Read an authoritative CodeMirror document at a logical UTF-16 position; only a changed document or another line prepares new Unicode output. */
  prepare(document: Text, position: number): GraphemeLine {
    if (
      this.current?.document === document &&
      this.current.from <= position &&
      position <= this.current.to
    )
      return this.current;
    const line = document.lineAt(position);
    this.current = {
      document,
      from: line.from,
      to: line.to,
      segments: segmenter.segment(line.text),
    };
    return this.current;
  }

  /** Source replacement and native view disposal release the one retained derived line and its document reference. */
  clear(): void {
    this.current = null;
  }
}

/** Resolve one whole grapheme or line break in logical-document UTF-16 source order, including a host-positioned interior caret. */
export function graphemeRange(
  cache: GraphemeLineCache,
  document: Text,
  position: number,
  forward: boolean,
): SourceRange | null {
  if (position === (forward ? document.length : 0)) return null;
  const line = cache.prepare(document, position);
  if (position === (forward ? line.to : line.from))
    return forward ? { from: position, to: position + 1 } : { from: position - 1, to: position };
  const segment = line.segments.containing(position - line.from - (forward ? 0 : 1));
  if (!segment) throw new Error('Native grapheme boundary unavailable');
  return {
    from: line.from + segment.index,
    to: line.from + segment.index + segment.segment.length,
  };
}

/** The mounted view owns derived segmentation lifetime; ordinary typing and selection updates prepare no Unicode output. */
class NativeGraphemes {
  readonly cache = new GraphemeLineCache();

  /** Retire old Unicode output on authoritative source changes without parsing or preparing another line. */
  update(update: ViewUpdate): void {
    if (update.docChanged) this.cache.clear();
  }

  /** Release the finite derived result when the editor is replaced or unmounted. */
  destroy(): void {
    this.cache.clear();
  }
}

/** Install the single mounted Unicode boundary owner alongside native editing. */
export const nativeGraphemes = ViewPlugin.fromClass(NativeGraphemes);

/** Delegate only Unicode boundary calculation at a logical UTF-16 position; CodeMirror retains document, caret, atomic ranges and history. */
export function nativeGraphemeRange(
  view: EditorView,
  position: number,
  forward: boolean,
): SourceRange | null {
  const owner = view.plugin(nativeGraphemes);
  if (!owner) throw new Error('Native grapheme owner unavailable');
  return graphemeRange(owner.cache, view.state.doc, position, forward);
}

/** Count UTF-16 units in the first and last native clusters of CodeMirror's bounded visual step, without retaining another source representation. */
export function nativeGraphemeWidths(text: string): {
  readonly first: number;
  readonly last: number;
} {
  const segments = segmenter.segment(text);
  return {
    first: segments.containing(0)?.segment.length ?? 0,
    last: segments.containing(text.length - 1)?.segment.length ?? 0,
  };
}
