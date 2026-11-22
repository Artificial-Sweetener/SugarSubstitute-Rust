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
/** Describe lightweight workflow presentation choices shared by every canvas host. */
export interface WorkflowViewSnapshot {
  readonly canvas: boolean;
  readonly masonry: boolean;
  /** The editor's width beside the output canvas in CSS pixels; zero selects the default width. */
  readonly editorWidth: number;
}

/**
 * Where a workflow's masonry was scrolled, kept by the Cube at its top so it lands on the same
 * Cube however the cards reflow in another window size.
 */
export interface WorkflowScroll {
  /** The Cube whose section held the top of the view, or null above every Cube. */
  readonly cube: string | null;
  /** CSS pixels from that section's top, or from the masonry's top, to the top of the view. */
  readonly offset: number;
}

/** One workflow's remembered scroll, bound to the workflow that was shown when it was taken. */
export interface WorkflowScrollMemory {
  read(): WorkflowScroll | null;
  record(scroll: WorkflowScroll): void;
  /** Hear when a restored session replaces the kept scroll, so a shown masonry can follow it. */
  subscribe(restored: () => void): () => void;
}

/** Accept a scroll from storage or a caller: a bounded offset and a Cube id or none. */
export function isWorkflowScroll(value: unknown): value is WorkflowScroll {
  if (typeof value !== 'object' || value === null) return false;
  const { cube, offset } = value as { cube?: unknown; offset?: unknown };
  return (
    (cube === null || (typeof cube === 'string' && cube.length > 0 && cube.length <= 256)) &&
    typeof offset === 'number' &&
    Number.isFinite(offset) &&
    offset >= 0 &&
    offset <= 10_000_000
  );
}

/** Expose accepted workflow view choices independently of rendered surfaces. */
export interface WorkflowViewSource {
  readonly snapshot: WorkflowViewSnapshot;
  readonly ready: boolean;
  subscribe(listener: () => void): () => void;
  change(value: Partial<WorkflowViewSnapshot>): void;
}

/** Give the masonry the shown workflow's scroll memory. */
export interface WorkflowScrollSource {
  /**
   * The shown workflow's scroll memory; it keeps answering for that workflow after another one
   * is shown. Null while no workflow is shown.
   */
  bindScroll(): WorkflowScrollMemory | null;
}
