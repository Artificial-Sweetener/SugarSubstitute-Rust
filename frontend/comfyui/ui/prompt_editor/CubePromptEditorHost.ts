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
/** Own Cube prompt editor sessions independently of renderer-specific native controls. */
import type { ComfyWidget } from '../types/graph.js';
import type { ModelCatalogClient } from '../model_library/ModelCatalogClient.js';
import type { ModelCatalogSnapshot } from '../model_library/ModelCatalog.js';
import { PromptWidgetBinding } from './PromptWidgetBinding.js';
import { preparePromptLoraCatalog } from './PromptLoraCatalog.js';
import { registerPromptEditorSurfaceOwner } from './PromptEditorSurface.js';
import type { PromptEditorSurface, PromptEditorSurfaceOwner } from './PromptEditorSurface.js';
import type { SugarPromptEditor } from './runtime/SugarPromptRuntime.js';
import type { LoraResourceAdapter } from '../../../sugarprompt/services/src/index.js';
import type { AppearanceSource } from '../surface/appearance/AppearanceState.js';
import {
  setCubePromptWidgetHeight,
  clearCubePromptWidgetHeight,
} from '../surface/CubePromptWidgetHeightStore.js';

interface PromptSession {
  creating: boolean;
  readonly binding: PromptWidgetBinding;
  readonly surfaces: Set<PromptEditorSurface>;
  editor: SugarPromptEditor | null;
  active: PromptEditorSurface | null;
  disposed: boolean;
}

/** Share native documents while renderer mounts and workflow ownership remain explicit. */
export class CubePromptEditorHost implements PromptEditorSurfaceOwner {
  readonly #document: Document;
  readonly #catalog: Pick<ModelCatalogClient, 'load' | 'watch' | 'prioritize'>;
  readonly #report: (error: unknown) => void;
  readonly #sessions = new Map<ComfyWidget, PromptSession>();
  readonly #restoreSurfaces = new Map<PromptEditorSurface, () => void>();
  #release: (() => void) | null = null;
  #catalogSnapshot: object | null = null;
  #loraCatalog: LoraResourceAdapter | null = null;
  #stopCatalog: (() => void) | null = null;

  /** Inject existing model catalog and feedback ownership. */
  constructor(
    document: Document,
    catalog: Pick<ModelCatalogClient, 'load' | 'watch' | 'prioritize'>,
    report: (error: unknown) => void,
    private readonly appearance: AppearanceSource,
  ) {
    this.#document = document;
    this.#catalog = catalog;
    this.#report = report;
  }

  /** Enable prompt enhancement before the host renders Cube fields. */
  setup(): void {
    this.#release ??= registerPromptEditorSurfaceOwner(this.#document, this);
  }

  /** Acquire a field presentation without duplicating its native editor or serialized value. */
  mount(surface: PromptEditorSurface): () => void {
    const hidden = surface.textarea.hidden;
    const display = surface.textarea.style.getPropertyValue('display');
    const priority = surface.textarea.style.getPropertyPriority('display');
    const restore = (): void => {
      if (
        typeof surface.widget.value === 'string' &&
        surface.textarea.value !== surface.widget.value
      )
        surface.textarea.value = surface.widget.value;
      surface.textarea.hidden = hidden;
      if (display) surface.textarea.style.setProperty('display', display, priority);
      else surface.textarea.style.removeProperty('display');
    };
    this.#restoreSurfaces.set(surface, restore);
    let session = this.#sessions.get(surface.widget);
    if (!session) {
      const created: PromptSession = {
        binding: new PromptWidgetBinding(surface.node, surface.widget, (source) => {
          created.editor?.refresh(source, this.#readOnly(created.active));
        }),
        surfaces: new Set(),
        editor: null,
        active: surface,
        disposed: false,
        creating: false,
      };
      session = created;
      this.#sessions.set(surface.widget, session);
    }
    session.surfaces.add(surface);
    session.active = surface;
    if (!session.editor && !session.creating) {
      session.creating = true;
      void this.#create(session, surface)
        .catch(this.#report)
        .finally(() => {
          session.creating = false;
        });
    }
    this.#present(session, surface);
    return () => {
      if (session.disposed) return;
      session.surfaces.delete(surface);
      this.#restoreSurfaces.get(surface)?.();
      this.#restoreSurfaces.delete(surface);
      if (session.active === surface) {
        session.active = [...session.surfaces].at(-1) ?? null;
        if (session.active) this.#present(session, session.active);
        else session.editor?.element.remove();
      }
    };
  }

  /** Refresh retained sessions and report workflow retirement to their renderer owner. */
  refresh(surface: PromptEditorSurface): boolean {
    const session = this.#sessions.get(surface.widget);
    if (!session || !session.surfaces.has(surface)) return false;
    session.editor?.refresh(session.binding.read(), this.#readOnly(surface));
    if (session.active === surface) this.#present(session, surface);
    return true;
  }

  /** Retire native documents before replacing an entire workflow. */
  beforeConfigureGraph(): void {
    this.#stopCatalog?.();
    this.#stopCatalog = null;
    for (const [widget, session] of this.#sessions) {
      session.disposed = true;
      session.editor?.dispose();
      session.binding.dispose();
      clearCubePromptWidgetHeight(widget);
      for (const surface of session.surfaces) this.#restoreSurfaces.get(surface)?.();
    }
    this.#sessions.clear();
    this.#restoreSurfaces.clear();
  }

  /** Release the document registration and every editor-owned resource. */
  dispose(): void {
    this.#release?.();
    this.#release = null;
    this.beforeConfigureGraph();
  }

  /** Load the bundled engine once and adapt the current shared model snapshot outside input. */
  async #create(session: PromptSession, surface: PromptEditorSurface): Promise<void> {
    const { SugarPromptEditor } = await import('./runtime/SugarPromptRuntime.js');
    if (session.disposed || !session.active) return;
    session.editor = new SugarPromptEditor(this.#document, {
      anchor: session.active.textarea,
      source: session.binding.read(),
      label: surface.node.title ?? 'Cube prompt',
      readOnly: this.#readOnly(session.active),
      commit: (source) => session.binding.commit(source),
      layout: (height) => {
        if (setCubePromptWidgetHeight(surface.widget, height + 20))
          session.active?.geometryChanged();
      },
      report: this.#report,
      appearance: this.appearance,
    });
    if (session.active) this.#present(session, session.active);
    const snapshot = await this.#catalog.load();
    if (session.disposed) return;
    this.#publishCatalog(snapshot);
    this.#stopCatalog ??= this.#catalog.watch((next) => this.#publishCatalog(next), this.#report);
    if (this.#loraCatalog) session.editor.setLoraCatalog(this.#loraCatalog);
  }

  /** Refresh installed identities and decoded media from the existing model catalog lifecycle. */
  #publishCatalog(snapshot: ModelCatalogSnapshot): void {
    if (snapshot !== this.#catalogSnapshot) {
      this.#catalogSnapshot = snapshot;
      this.#loraCatalog = preparePromptLoraCatalog(snapshot, this.#document.baseURI, (items) => {
        if (items.length) void this.#catalog.prioritize(items).catch(this.#report);
      });
      for (const current of this.#sessions.values())
        current.editor?.setLoraCatalog(this.#loraCatalog);
    }
  }

  /** Hide only the native editing control while retaining its host-facing value contract. */
  #present(session: PromptSession, surface: PromptEditorSurface): void {
    if (!session.editor || session.disposed) return;
    session.editor.attach(surface.textarea);
    if (!surface.textarea.hidden) surface.textarea.hidden = true;
    if (
      surface.textarea.style.getPropertyValue('display') !== 'none' ||
      surface.textarea.style.getPropertyPriority('display') !== 'important'
    )
      surface.textarea.style.setProperty('display', 'none', 'important');
  }

  /** Native permissions remain authoritative across every presentation. */
  #readOnly(surface: PromptEditorSurface | null): boolean {
    return (
      !surface ||
      surface.hostReadOnly() ||
      surface.textarea.readOnly ||
      surface.textarea.disabled ||
      surface.widget.computedDisabled === true
    );
  }
}
