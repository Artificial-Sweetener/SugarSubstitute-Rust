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
/** Reproduce Substitute's justified image-wall partition and row-height policy. */
export interface ModelTileBox {
  readonly index: number;
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}

export interface ModelWallRow {
  readonly top: number;
  readonly height: number;
  readonly tiles: readonly ModelTileBox[];
}

interface RowCandidate {
  start: number;
  end: number;
  height: number;
  widths: number[];
  cost: number;
}

export interface ModelWallProfile {
  readonly targetHeight: number;
  readonly minimumHeight: number;
  readonly maximumHeight: number;
  readonly minimumTileWidth: number;
}
/** Match the standard Substitute model-picker wall independently of detector chooser geometry. */
export const MODEL_PICKER_WALL_PROFILE: ModelWallProfile = {
  targetHeight: 198,
  minimumHeight: 156,
  maximumHeight: 258,
  minimumTileWidth: 126,
};
/** Match Substitute's packaged detector-thumbnail chooser independently from model gallery sizing. */
export const MODEL_DETECTOR_WALL_PROFILE: ModelWallProfile = {
  targetHeight: 184,
  minimumHeight: 144,
  maximumHeight: 224,
  minimumTileWidth: 130,
};
/** Keep the same compact image-to-image spacing as Substitute's wall. */
export const MODEL_WALL_GUTTER = 4;

/** Normalize absent or pathological dimensions using Substitute's portrait fallback. */
function aspect(value: number): number {
  return Number.isFinite(value) && value > 0 ? Math.min(6, Math.max(0.2, value)) : 0.72;
}

/** Solve complete rows while keeping the final row comfortable rather than stretched. */
export function layoutModelWall(
  aspects: readonly number[],
  width: number,
  profile: ModelWallProfile = MODEL_PICKER_WALL_PROFILE,
): readonly ModelWallRow[] {
  if (!aspects.length || !Number.isFinite(width) || width <= 0) return [];
  const values = aspects.map(aspect);
  const count = Math.max(
    1,
    Math.floor((width + MODEL_WALL_GUTTER) / (profile.minimumTileWidth + MODEL_WALL_GUTTER)),
  );
  const candidates =
    solve(values, width, count, false, profile) ?? solve(values, width, count, true, profile) ?? [];
  let top = 0;
  return candidates.map((candidate) => {
    let left = 0;
    const row: ModelWallRow = {
      top,
      height: candidate.height,
      tiles: candidate.widths.map((tileWidth, offset) => {
        const tile: ModelTileBox = {
          index: candidate.start + offset,
          left,
          top,
          width: tileWidth,
          height: candidate.height,
        };
        left += tileWidth + MODEL_WALL_GUTTER;
        return tile;
      }),
    };
    top += candidate.height + MODEL_WALL_GUTTER;
    return row;
  });
}

/** Evaluate bounded contiguous partitions with the original trailing-row penalties. */
function solve(
  values: readonly number[],
  width: number,
  count: number,
  relaxed: boolean,
  profile: ModelWallProfile,
): RowCandidate[] | null {
  const costs = new Array<number>(values.length + 1).fill(Infinity);
  const choices = new Array<RowCandidate | null>(values.length + 1).fill(null);
  costs[values.length] = 0;
  for (let start = values.length - 1; start >= 0; start--) {
    for (let end = start + 1; end <= Math.min(values.length, start + count); end++) {
      const ratios = values.slice(start, end);
      const available = width - (ratios.length - 1) * MODEL_WALL_GUTTER;
      const natural =
        available /
        Math.max(
          0.001,
          ratios.reduce((sum, value) => sum + value, 0),
        );
      const final = end === values.length;
      const bounded = natural < profile.minimumHeight || natural > profile.maximumHeight;
      const candidates: RowCandidate[] = [];
      if (!bounded || (!final && relaxed)) {
        const height = Math.min(profile.maximumHeight, Math.max(profile.minimumHeight, natural));
        const widths = ratios.map((ratio) => Math.max(1, ratio * height));
        const last = widths.length - 1;
        widths[last] = Math.max(
          1,
          (widths[last] ?? 1) + available - widths.reduce((sum, value) => sum + value, 0),
        );
        const coverage =
          (widths.reduce((sum, value) => sum + value, 0) +
            (widths.length - 1) * MODEL_WALL_GUTTER) /
          width;
        if (final || Math.abs(coverage - 1) <= 0.01) {
          candidates.push({
            start,
            end,
            height,
            widths,
            cost: final
              ? finalCost(height, coverage, ratios.length, width, false, profile)
              : Math.abs(height - profile.targetHeight) +
                Math.abs(height - natural) * 12 +
                (ratios.length === 1 ? 24 : 0),
          });
        }
      }
      if (final) {
        const height = Math.min(
          profile.maximumHeight,
          Math.max(1, Math.min(profile.targetHeight, natural)),
        );
        const widths = ratios.map((ratio) => Math.max(1, ratio * height));
        const coverage =
          (widths.reduce((sum, value) => sum + value, 0) +
            (widths.length - 1) * MODEL_WALL_GUTTER) /
          width;
        candidates.push({
          start,
          end,
          height,
          widths,
          cost: finalCost(height, coverage, ratios.length, width, true, profile),
        });
      }
      for (const candidate of candidates) {
        const next = choices[end];
        const transition = next
          ? Math.abs(candidate.height - next.height) * (next.end === values.length ? 1.1 : 0.18)
          : 0;
        const cost = candidate.cost + transition + (costs[end] ?? Infinity);
        if (cost < (costs[start] ?? Infinity) - 0.001) {
          costs[start] = cost;
          choices[start] = candidate;
        }
      }
    }
  }
  if (!Number.isFinite(costs[0])) return null;
  const rows: RowCandidate[] = [];
  for (let index = 0; index < values.length; ) {
    const choice = choices[index];
    if (!choice) return null;
    rows.push(choice);
    index = choice.end;
  }
  return rows;
}

/** Prefer full, readable trailing rows with Substitute's weighted comfort policy. */
function finalCost(
  height: number,
  coverage: number,
  count: number,
  width: number,
  ragged: boolean,
  profile: ModelWallProfile,
): number {
  const comfort = Math.max(profile.minimumHeight * 0.9, profile.targetHeight * 0.82);
  return (
    Math.abs(height - profile.targetHeight) +
    Math.max(0, (1 - coverage) * width) * 0.18 +
    Math.max(0, comfort - height) * 6.5 +
    Math.max(0, count - 7) * 65 +
    (ragged ? 18 : 0)
  );
}

/** Limit mounted DOM to nearby rows while preserving the full scrollable wall height. */
export function visibleModelRows(
  rows: readonly ModelWallRow[],
  scrollTop: number,
  height: number,
): readonly ModelWallRow[] {
  const overscan = MODEL_PICKER_WALL_PROFILE.maximumHeight * 2;
  return rows.filter(
    (row) =>
      row.top + row.height >= scrollTop - overscan && row.top <= scrollTop + height + overscan,
  );
}
