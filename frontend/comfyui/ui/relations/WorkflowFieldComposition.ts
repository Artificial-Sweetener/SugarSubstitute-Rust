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
/** Compose native field appearance and relation policy around one submitted seed history. */
import { RelationHostLifecycle, type RelationHostOptions } from './RelationHostLifecycle.js';
import { FluentAppearanceHost } from '../surface/appearance/FluentAppearanceHost.js';

export interface WorkflowFieldHostOptions
  extends Omit<RelationHostOptions, 'reportError' | 'appearance'> {
  reportRelationError(error: unknown): void;
  reportAppearanceError(error: unknown): void;
}

/** Share seed reuse across actual native field mounts and the override toolbar. */
export function createWorkflowFieldHost(options: WorkflowFieldHostOptions): {
  appearance: FluentAppearanceHost;
  relations: RelationHostLifecycle;
} {
  const relations: RelationHostLifecycle = new RelationHostLifecycle({
    ...options,
    reportError: options.reportRelationError,
    appearance: () => appearance.state,
  });
  const appearance: FluentAppearanceHost = new FluentAppearanceHost({
    app: options.app,
    document: options.document,
    seedReuse: (node, name) => relations.seedReuse.field(node, name),
    reportError: options.reportAppearanceError,
  });
  return { appearance, relations };
}
