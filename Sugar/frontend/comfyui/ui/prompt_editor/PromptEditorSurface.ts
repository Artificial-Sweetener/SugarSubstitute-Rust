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
/** Publish identified Cube prompt mounts without discovering graphs from editor DOM activity. */
import type { ComfyNode, ComfyWidget } from '../types/graph.js';

/** Renderers retain graph identity and supply only their exact editable prompt surface. */
export interface PromptEditorSurface {
  readonly node: ComfyNode;
  readonly widget: ComfyWidget;
  readonly textarea: HTMLTextAreaElement;
  readonly geometryChanged: () => void;
  readonly hostReadOnly: () => boolean;
}

/** One document's prompt component owns all registered renderer presentations. */
export interface PromptEditorSurfaceOwner {
  /** Return the presentation release, or null when the native control must remain authoritative. */
  mount(surface: PromptEditorSurface): (() => void) | null;
  refresh(surface: PromptEditorSurface): boolean;
  ready(root: HTMLElement): Promise<void>;
  dispose(): void;
}

const owners = new WeakMap<Document, PromptEditorSurfaceOwner>();

/** Register the focused component boundary before native Cube cards mount. */
export function registerPromptEditorSurfaceOwner(
  document: Document,
  owner: PromptEditorSurfaceOwner,
): () => void {
  const previous = owners.get(document);
  if (previous && previous !== owner) previous.dispose();
  owners.set(document, owner);
  return () => {
    if (owners.get(document) === owner) owners.delete(document);
  };
}

/** Mount through an identified renderer boundary; tests and uninitialized hosts retain native controls. */
export function mountPromptEditorSurface(surface: PromptEditorSurface): (() => void) | null {
  return owners.get(surface.textarea.ownerDocument)?.mount(surface) ?? null;
}

/** Report expired sessions so retained renderer surfaces can reacquire their editor. */
export function refreshPromptEditorSurface(surface: PromptEditorSurface): boolean {
  return owners.get(surface.textarea.ownerDocument)?.refresh(surface) ?? false;
}

/** Finish identified prompt construction before its owning presentation is exposed. */
export function whenPromptEditorsPresented(root: HTMLElement): Promise<void> {
  return owners.get(root.ownerDocument)?.ready(root) ?? Promise.resolve();
}
