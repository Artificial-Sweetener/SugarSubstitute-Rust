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
/** Retain immutable view choices independently of graph objects, history, and rendered resources. */
import type { WorkflowViewSnapshot } from '../domain/WorkflowView.js';

interface WorkflowViewBinding {
  readonly id: string;
  readonly path: string | null;
}

/**
 * The choices browser storage keeps per saved workflow. Whether a workflow shows masonry is not
 * among them: the workflow records that itself.
 */
interface StoredWorkflowView {
  readonly canvas: boolean;
  readonly editorWidth: number;
}

/** What a workflow opens with when nothing about it is stored or live yet. */
export interface WorkflowViewDefaults {
  readonly canvas: boolean;
  readonly masonry: boolean;
}

/** Accept stored or requested editor widths in CSS pixels, with zero selecting the default. */
function isEditorWidth(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 && value <= 16384;
}

/** Bound persistent history while retaining every live workflow's lightweight choices. */
export class WorkflowViewPreferences {
  readonly #live = new Map<string, { path: string | null; view: WorkflowViewSnapshot }>();
  readonly #stored = new Map<string, StoredWorkflowView>();
  #selected: string | null = null;
  #snapshot: WorkflowViewSnapshot = Object.freeze({ canvas: true, masonry: false, editorWidth: 0 });

  /** Validate the persistence boundary once and keep tab activation entirely in memory. */
  constructor(
    value: unknown,
    private readonly write: (
      value: readonly ({ readonly path: string } & StoredWorkflowView)[],
    ) => void,
  ) {
    if (!Array.isArray(value)) return;
    for (const item of value.slice(-128)) {
      if (
        typeof item !== 'object' ||
        item === null ||
        !('path' in item) ||
        !('canvas' in item) ||
        typeof item.path !== 'string' ||
        !item.path ||
        item.path.length > 4096 ||
        typeof item.canvas !== 'boolean'
      )
        continue;
      this.#stored.set(
        item.path,
        Object.freeze({
          canvas: item.canvas,
          // Entries written before editor widths were stored select the default width.
          editorWidth:
            'editorWidth' in item && isEditorWidth(item.editorWidth) ? item.editorWidth : 0,
        }),
      );
    }
  }

  /** Read the existing accepted object rather than reconstructing settings during activation. */
  get snapshot(): WorkflowViewSnapshot {
    return this.#snapshot;
  }

  /** Capture one explicit live occurrence, including temporary workflows without browser-history keys. */
  snapshotFor(
    binding: WorkflowViewBinding,
    defaults: () => WorkflowViewDefaults,
  ): WorkflowViewSnapshot {
    const current = this.#live.get(binding.id);
    if (current) return current.view;
    const view = this.#opening(binding, defaults);
    this.#live.set(binding.id, { path: binding.path, view });
    return view;
  }

  /** Import native checkpoint choices without publishing browser persistence ahead of host durability. */
  restore(binding: WorkflowViewBinding, snapshot: WorkflowViewSnapshot): void {
    if (
      typeof snapshot.canvas !== 'boolean' ||
      typeof snapshot.masonry !== 'boolean' ||
      !isEditorWidth(snapshot.editorWidth)
    )
      throw new TypeError('Restored workflow presentation is invalid.');
    const view = Object.freeze({ ...snapshot });
    this.#live.set(binding.id, { path: binding.path, view });
    if (this.#selected === binding.id) this.#snapshot = view;
  }

  /** Select an explicit committed binding and carry choices through rename or Save As. */
  activate(binding: WorkflowViewBinding, defaults: () => WorkflowViewDefaults): boolean {
    let current = this.#live.get(binding.id);
    if (!current) {
      const stored = binding.path ? this.#stored.get(binding.path) : undefined;
      current = { path: binding.path, view: this.#opening(binding, defaults) };
      this.#live.set(binding.id, current);
      if (!stored) this.#persist(current);
    } else if (current.path !== binding.path) {
      current.path = binding.path;
      this.#persist(current);
    }
    const changed = this.#selected !== binding.id || this.#snapshot !== current.view;
    this.#selected = binding.id;
    this.#snapshot = current.view;
    return changed;
  }

  /**
   * Record only actual user choices without changing workflow content or graph history.
   *
   * @throws RangeError when an editor width is outside the persisted range.
   */
  change(value: Partial<WorkflowViewSnapshot>): boolean {
    if (value.editorWidth !== undefined && !isEditorWidth(value.editorWidth))
      throw new RangeError('Workflow editor width is invalid.');
    const current = this.#selected ? this.#live.get(this.#selected) : null;
    if (!current) return false;
    const next = { ...current.view, ...value };
    if (
      next.canvas === current.view.canvas &&
      next.masonry === current.view.masonry &&
      next.editorWidth === current.view.editorWidth
    )
      return false;
    current.view = Object.freeze(next);
    this.#snapshot = current.view;
    this.#persist(current);
    return true;
  }

  /**
   * Release closed in-memory bindings while keeping bounded reopen metadata. A closed workflow's
   * stored choices follow what it last showed, including ones a session restore supplied without
   * publishing them, so the workflow reopens the way it was left.
   */
  retain(ids: readonly string[]): void {
    const open = new Set(ids);
    for (const [id, closed] of this.#live) {
      if (open.has(id)) continue;
      this.#live.delete(id);
      const stored = closed.path ? this.#stored.get(closed.path) : undefined;
      if (
        closed.path &&
        (stored?.canvas !== closed.view.canvas || stored.editorWidth !== closed.view.editorWidth)
      )
        this.#persist(closed);
    }
    if (this.#selected && !open.has(this.#selected)) this.#selected = null;
  }

  /** The choices a workflow opens with: stored canvas and width, and its own masonry default. */
  #opening(
    binding: WorkflowViewBinding,
    defaults: () => WorkflowViewDefaults,
  ): WorkflowViewSnapshot {
    const stored = binding.path ? this.#stored.get(binding.path) : undefined;
    const fallback = defaults();
    return Object.freeze({
      canvas: stored?.canvas ?? fallback.canvas,
      masonry: fallback.masonry,
      editorWidth: stored?.editorWidth ?? 0,
    });
  }

  /** Persist changed named choices; ordinary tab switching performs no storage writes. */
  #persist(current: { path: string | null; view: WorkflowViewSnapshot }): void {
    if (!current.path) return;
    const stored = this.#stored.get(current.path);
    if (stored?.canvas === current.view.canvas && stored.editorWidth === current.view.editorWidth)
      return;
    this.#stored.delete(current.path);
    this.#stored.set(
      current.path,
      Object.freeze({ canvas: current.view.canvas, editorWidth: current.view.editorWidth }),
    );
    while (this.#stored.size > 128) {
      const oldest = this.#stored.keys().next().value;
      if (oldest !== undefined) this.#stored.delete(oldest);
    }
    this.write([...this.#stored].map(([path, view]) => ({ path, ...view })));
  }
}
