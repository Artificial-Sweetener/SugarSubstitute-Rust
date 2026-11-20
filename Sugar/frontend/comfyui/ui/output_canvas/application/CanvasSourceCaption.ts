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
/** Name the canvas tab of a Cube's output: one rule for a live graph and for a restored one. */
import type {
  CubeOutputFacts,
  WorkflowOutputSource,
} from '../../execution/domain/WorkflowOutput.js';

/** What a tab shows for one Cube output, and the full path its tooltip and provenance keep. */
export interface CubeOutputCaption {
  readonly label: string;
  readonly breadcrumb: readonly string[];
}

/**
 * Caption a Cube's output with the Cube's own name.
 *
 * A Cube with several outputs adds the output's name so its tabs differ. `ancestors` are the titles of the graphs the Cube sits in.
 */
export function cubeOutputCaption(
  facts: CubeOutputFacts,
  ancestors: readonly string[] = [],
): CubeOutputCaption {
  const outputName = facts.outputName || 'Output';
  return {
    label: facts.outputCount > 1 ? `${facts.name} \u00b7 ${outputName}` : facts.name,
    breadcrumb: [...ancestors, facts.title, outputName],
  };
}

/**
 * Caption every Cube output of a decoded workflow for the canvas.
 *
 * A workflow decoded from its saved form names a Cube's output by the output alone, which is
 * what a run's provenance needs; the canvas shows the Cube's name instead. Sources that are not
 * Cube outputs are returned as they are.
 */
export function presentCanvasSources(
  sources: readonly WorkflowOutputSource[],
): readonly WorkflowOutputSource[] {
  return Object.freeze(
    sources.map((source) =>
      source.cube
        ? Object.freeze({
            ...source,
            ...cubeOutputCaption(source.cube, source.breadcrumb.slice(0, -1)),
          })
        : source,
    ),
  );
}
