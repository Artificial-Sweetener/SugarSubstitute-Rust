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
/** Retain workflow field interactions independently of replaceable node-card presentations. */
import type { ModelCatalogSnapshot } from './ModelCatalog.js';
import type { ModelPickerBinding } from './ModelPickerBinding.js';
import { readComfyModelFields } from './ComfyModelFieldAdapter.js';
import { comfyModelFieldOwner } from './ComfyModelFieldOwner.js';
import type { ComfyModelFieldOwner } from './ComfyModelFieldOwner.js';
import { ModelPickerSession } from './ModelPickerSession.js';

interface FieldSession {
  readonly owner: ComfyModelFieldOwner;
  readonly roots: Set<HTMLElement>;
  readonly session: ModelPickerSession;
}

/** Keep transfers attached to actual graph nodes across masonry and appearance remounts. */
export class ModelPickerSessions {
  readonly #document: Document;
  readonly #snapshot: () => ModelCatalogSnapshot | null;
  readonly #app: unknown;
  readonly #owners = new Map<object, Map<string, FieldSession>>();
  /** Bind the current workflow and catalog used to validate legal choices at acceptance time. */
  constructor(document: Document, snapshot: () => ModelCatalogSnapshot | null, app: unknown) {
    this.#document = document;
    this.#snapshot = snapshot;
    this.#app = app;
  }
  /** Reattach a replacement presentation to the same workflow-owned interaction. */
  get(root: HTMLElement, binding: ModelPickerBinding): ModelPickerSession {
    const owner = comfyModelFieldOwner(this.#app, root, binding);
    let fields = this.#owners.get(owner.identity);
    if (!fields) {
      fields = new Map();
      this.#owners.set(owner.identity, fields);
    }
    const inputName = binding.inputName ?? binding.label;
    const kind = binding.kind;
    const key = JSON.stringify([inputName, kind]);
    let field = fields.get(key);
    if (!field) {
      const roots = new Set<HTMLElement>();
      const session = new ModelPickerSession(this.#document, () => {
        if (!owner.exists()) return null;
        const snapshot = this.#snapshot();
        if (snapshot)
          for (const current of roots) {
            if (!current.isConnected) continue;
            const receiver = readComfyModelFields(current, snapshot).find(
              (candidate) => candidate.inputName === inputName && candidate.kind === kind,
            );
            if (receiver) return receiver;
          }
        return owner.receiver();
      });
      field = { owner, roots, session };
      fields.set(key, field);
    }
    field.roots.add(root);
    return field.session;
  }
  /** Prune detached presentations while cancelling only deleted or replaced workflow owners. */
  reconcile(): void {
    for (const [identity, fields] of this.#owners) {
      for (const [key, field] of fields) {
        for (const root of field.roots) if (!root.isConnected) field.roots.delete(root);
        if (field.owner.exists()) continue;
        field.session.dispose();
        fields.delete(key);
      }
      if (!fields.size) this.#owners.delete(identity);
    }
  }
  /** Cancel interactions when the supported renderer or enhancement is explicitly disabled. */
  clear(): void {
    for (const fields of this.#owners.values())
      for (const field of fields.values()) field.session.dispose();
    this.#owners.clear();
  }
}
