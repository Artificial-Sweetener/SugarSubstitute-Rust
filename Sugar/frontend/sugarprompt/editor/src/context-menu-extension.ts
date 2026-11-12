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
/** Native state assembly injects configuration authority and connects the single interaction and tooltip owners. */
import { Facet, type Extension } from '@codemirror/state';
import { ViewPlugin, type EditorView } from '@codemirror/view';
import { NativeContextMenu } from './context-menu.js';
import { promptMenuPresentation } from './menu-presentation.js';
import type { PromptMenuFactory } from './menu-presentation.js';
/** Public controller delegates to this exact mounted plugin rather than constructing an aggregate owner. */
export const nativeMenuOwner = Facet.define<
  (view: EditorView) => NativeContextMenu | null,
  ((view: EditorView) => NativeContextMenu | null) | null
>({
  combine: (values) => {
    if (values.length > 1) throw new Error('Multiple native menu owners');
    return values[0] ?? null;
  },
});

/** The mounted prompt composes native rows and forwards mode intent to its existing atomic host owner. */
export function nativeContextMenu(
  changeRichRendering: (enabled: boolean) => void,
  presentation?: PromptMenuFactory,
): Extension {
  const plugin = ViewPlugin.define((view) => new NativeContextMenu(view, changeRichRendering), {
    eventHandlers: {
      /** Prepared right-button intent preserves directional native selection. */
      mousedown(event) {
        return this.pointer(event);
      },
      /** One native interaction owner measures and opens the literal menu. */
      contextmenu(event) {
        return this.context(event);
      },
      /** Native opening and dismissal keys delegate to the same finite owner. */
      keydown(event) {
        return this.key(event);
      },
    },
  });
  const owner = (view: EditorView) => view.plugin(plugin);
  return [
    plugin,
    nativeMenuOwner.of(owner),
    ...(presentation ? [promptMenuPresentation(owner, presentation)] : []),
  ];
}
