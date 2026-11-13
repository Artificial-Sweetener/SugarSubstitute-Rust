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
/** Prepared scene decorations conceal markers while keeping title text, selection and history native. */
import { requireValue } from '../../core/src/required.js';
import { type Range, type EditorState } from '@codemirror/state';
import { Decoration } from '@codemirror/view';
import {
  prepareSceneHeadings,
  type SceneDocument,
  type SceneHeading,
} from '../../core/src/index.js';
import { sourcePositionsToDocument } from '../../core/src/line-endings.js';
import { lineEndings } from './exact-source.js';
import { sceneErrors } from './scene-errors.js';
import { SourceToken, SourceWidget } from './projection-widgets.js';
/** A zero-width native widget retains a concealed source prefix without displaying another editable title. */
class SceneMarkerWidget extends SourceWidget {
  /** Marker display is identical across source snapshots; CodeMirror can retain its actual DOM. */
  override eq(other: SceneMarkerWidget): boolean {
    return other instanceof SceneMarkerWidget;
  }
  /** Construct inert zero-width chrome; native pointer routing remains inherited from SourceWidget. */
  override toDOM(): HTMLElement {
    const marker = document.createElement('span');
    marker.className = 'sp-scene-marker';
    marker.setAttribute('aria-hidden', 'true');
    return marker;
  }
}
/** Only the canonical empty Scene prefix can be split across a readonly carrier boundary without duplicating ink or resource ownership. */
export function isSceneMarkerWidget(value: unknown): boolean {
  return value instanceof SceneMarkerWidget;
}
/** One source-bound heading publication supplies rendering and syntax ownership outside input/paint. */
export interface SceneProjection {
  readonly decorations: readonly Range<Decoration>[];
  readonly tokens: readonly Range<SourceToken>[];
}
/** Prepare literal editable title marks and concealed prefix widgets from cached scene semantics. */
export function prepareSceneProjection(
  state: EditorState,
  document: SceneDocument,
): SceneProjection {
  const headings = prepareSceneHeadings(document, state.field(sceneErrors).keys);
  const positions = sourcePositionsToDocument(
    state.doc.toString(),
    state.field(lineEndings),
    headings.flatMap(({ concealedRange, visibleRange }) => [
      concealedRange.from,
      concealedRange.to,
      visibleRange.to,
    ]),
  );
  return compileSceneHeadingProjection(headings, positions);
}
/** Native and proposed-source compilation share one heading renderer; positions are ordered concealed-start/title-start/title-end UTF-16 operands. */
export function compileSceneHeadingProjection(
  headings: readonly SceneHeading[],
  positions: readonly number[],
): SceneProjection {
  if (positions.length !== headings.length * 3) throw new Error('scene-heading-position-count');
  const decorations: Range<Decoration>[] = [],
    tokens: Range<SourceToken>[] = [];
  for (const [index, heading] of headings.entries()) {
    const from = requireValue(positions[index * 3], 'scene-projection'),
      titleFrom = requireValue(positions[index * 3 + 1], 'scene-projection'),
      to = requireValue(positions[index * 3 + 2], 'scene-projection'),
      token = new SourceToken(false);
    decorations.push(
      Decoration.replace({
        widget: new SceneMarkerWidget(token),
        scene: true,
      }).range(from, titleFrom),
      Decoration.mark({
        class: 'sp-scene-title',
        scene: true,
        attributes: {
          'data-scene-error': String(heading.error),
          'aria-invalid': String(heading.error),
          ...(heading.error
            ? {
                title: heading.marker.duplicate
                  ? 'Duplicate scene title'
                  : 'Scene rejected by host',
              }
            : {}),
        },
      }).range(titleFrom, to),
    );
    tokens.push(token.range(from, to));
  }
  return {
    decorations,
    tokens,
  };
}
