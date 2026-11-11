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
/** Actual decoration/attribute output is transported into finite physical test nodes; this is not a native DOM renderer. */
import type { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import type { motionPaintFixture } from './reorder-motion-paint-fixture.ts';
/** Actual native chip declarations contain the literal class/attributes emitted by the sole preview owner. */
export interface NativeReorderChipMetadata {
  readonly className: string;
  readonly attributes: Readonly<Record<string, string>>;
}
/** Read actual declarations once per controlled draw, with no alternate source ranges, active policy or semantic roster. */
export function nativeReorderChipMetadata(
  state: EditorState,
): ReadonlyMap<string, NativeReorderChipMetadata> {
  const publication = state.field(reorderPreview),
    byId = new Map<string, NativeReorderChipMetadata>();
  if (!publication.native) return byId;
  publication.decorations.between(0, state.doc.length, (_from, _to, decoration) => {
    const source: unknown = decoration.spec.attributes,
      className: unknown = decoration.spec.class;
    if (!source || typeof source !== 'object' || !('data-reorder-chip' in source)) return;
    const id = source['data-reorder-chip'];
    if (typeof id !== 'string' || typeof className !== 'string')
      throw new Error('native-metadata-transport-declaration-invalid');
    const attributes: Record<string, string> = {};
    const record = source as Readonly<Record<string, unknown>>;
    for (const [name, value] of Object.entries(record)) {
      if (typeof value !== 'string') throw new Error('native-metadata-transport-attribute-invalid');
      attributes[name] = value;
    }
    byId.set(id, Object.freeze({ className, attributes: Object.freeze(attributes) }));
  });
  if (publication.value && !publication.value.order.order.every((id) => byId.has(String(id))))
    throw new Error('fixture-native-mark-metadata-unavailable');
  return byId;
}
/** Supported facet output is copied literally; first publication for an attribute matches the fixture's existing lookup precedence. */
export function nativeReorderRootMetadata(
  state: EditorState,
  view: EditorView,
): ReadonlyMap<string, string> {
  const result = new Map<string, string>();
  for (const provider of state.facet(EditorView.editorAttributes)) {
    const attributes = typeof provider === 'function' ? provider(view) : provider;
    if (attributes === null) continue;
    for (const [name, value] of Object.entries(attributes))
      if (!result.has(name)) result.set(name, value);
  }
  return result;
}
/** Copy exact owner-issued scope attributes and root presentation without reconstructing source ranges or the active/held policy. */
export function drawNativeReorderMetadata(
  fixture: ReturnType<typeof motionPaintFixture>,
  state: EditorState,
  roots: readonly Pick<HTMLElement, 'getAttribute' | 'setAttribute'>[] = fixture.chips,
): void {
  const publication = state.field(reorderPreview);
  if (!publication.native) return;
  const byId = nativeReorderChipMetadata(state);
  for (const node of roots) {
    const attributes = byId.get(node.getAttribute('data-reorder-chip') ?? '');
    if (!attributes) throw new Error('native-metadata-transport-chip-unavailable');
    for (const [name, value] of Object.entries(attributes.attributes))
      if (name === 'data-reorder-native-value') node.setAttribute(name, value);
  }
  for (const [name, value] of nativeReorderRootMetadata(state, fixture.view))
    if (name.startsWith('data-reorder-native-')) fixture.view.dom.setAttribute(name, value);
}
