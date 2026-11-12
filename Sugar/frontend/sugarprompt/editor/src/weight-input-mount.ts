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
/** CodeMirror widgets delegate native numeric input lifetime to the existing view interaction owner. */
import { Facet } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
/** The widget owns subtree removal; the interaction owner releases its input/listener lifetime. */
export interface WeightInputMount {
  /** Release finite native input ownership without mutating CodeMirror-managed parent DOM. */
  release(): void;
}
/** One supported native extension binds widget cells to the mounted view's numeric interaction. */
export type WeightInputFactory = (
  view: EditorView,
  key: symbol,
  cell: HTMLElement,
  value: string,
) => WeightInputMount;
/** Framework-independent native state receives one finite widget mount adapter. */
export const weightInputFactory = Facet.define<WeightInputFactory, WeightInputFactory | null>({
  /** Duplicate numeric owners are an observable configuration invariant. */
  combine(values) {
    if (values.length > 1) throw new Error('Duplicate native weight input owner');
    return values[0] ?? null;
  },
});
const mounted = new WeakMap<HTMLElement, WeightInputMount>();
/** Mount only when the installed view owner admits the current source-bound key. */
export function mountWeightInput(
  view: EditorView,
  key: symbol,
  cell: HTMLElement,
  value: string,
): void {
  const factory = view.state.facet(weightInputFactory);
  if (!factory) throw new Error('Native weight input owner missing');
  mounted.set(cell, factory(view, key, cell, value));
}
/** Native widget teardown releases its exact mount even if an equivalent widget reused the DOM. */
export function releaseWeightInput(cell: HTMLElement): void {
  const current = mounted.get(cell);
  mounted.delete(cell);
  current?.release();
}
