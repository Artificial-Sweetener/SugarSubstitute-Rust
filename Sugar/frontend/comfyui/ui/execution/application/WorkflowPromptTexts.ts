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
/** Write a run's prompt texts into the serialized workflow that is sent to run. */
import { isRecord } from '../../types/common.js';

/**
 * Return a workflow whose prompt widget values are replaced by the texts a run uses for them.
 *
 * `texts` maps a prompt text as the workflow holds it to the text the run uses instead: one
 * scene's text for a scene run, or a text with its wildcards resolved. Only whole widget values
 * equal to a mapped text change. A serialized Cube holds a prompt on its face and in its
 * definition, as ordered `widgets_values` and as SugarCubes' named widget values, and every one
 * of them is replaced. The workflow passed in is never modified.
 */
export function workflowWithPromptTexts(
  workflow: unknown,
  texts: ReadonlyMap<string, string>,
): unknown {
  const replaced = (item: unknown): unknown =>
    typeof item === 'string' ? (texts.get(item) ?? item) : item;
  const rewrite = (value: unknown, depth: number): unknown => {
    if (depth > 64) return value;
    if (Array.isArray(value)) return value.map((entry) => rewrite(entry, depth + 1));
    if (!isRecord(value)) return value;
    const copy: Record<string, unknown> = {};
    for (const [key, entry] of Object.entries(value))
      copy[key] =
        key === 'widgets_values' && Array.isArray(entry)
          ? entry.map(replaced)
          : key === 'sugarcubes_widget_values' && isRecord(entry)
            ? Object.fromEntries(
                Object.entries(entry).map(([name, item]) => [name, replaced(item)]),
              )
            : rewrite(entry, depth + 1);
    return copy;
  };
  return texts.size ? rewrite(workflow, 0) : workflow;
}
