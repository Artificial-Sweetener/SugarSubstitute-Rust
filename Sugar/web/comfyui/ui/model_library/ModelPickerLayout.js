/** Match the standard Substitute model-picker wall independently of detector chooser geometry. */
export const MODEL_PICKER_WALL_PROFILE = {
    targetHeight: 198,
    minimumHeight: 156,
    maximumHeight: 258,
    minimumTileWidth: 126,
};
/** Match Substitute's packaged detector-thumbnail chooser independently from model gallery sizing. */
export const MODEL_DETECTOR_WALL_PROFILE = {
    targetHeight: 184,
    minimumHeight: 144,
    maximumHeight: 224,
    minimumTileWidth: 130,
};
/** Keep the same compact image-to-image spacing as Substitute's wall. */
export const MODEL_WALL_GUTTER = 4;
/** Normalize absent or pathological dimensions using Substitute's portrait fallback. */
function aspect(value) {
    return Number.isFinite(value) && value > 0 ? Math.min(6, Math.max(0.2, value)) : 0.72;
}
/** Solve complete rows while keeping the final row comfortable rather than stretched. */
export function layoutModelWall(aspects, width, profile = MODEL_PICKER_WALL_PROFILE) {
    if (!aspects.length || !Number.isFinite(width) || width <= 0)
        return [];
    const values = aspects.map(aspect);
    const count = Math.max(1, Math.floor((width + MODEL_WALL_GUTTER) / (profile.minimumTileWidth + MODEL_WALL_GUTTER)));
    const candidates = solve(values, width, count, false, profile) ?? solve(values, width, count, true, profile) ?? [];
    let top = 0;
    return candidates.map((candidate) => {
        let left = 0;
        const row = {
            top,
            height: candidate.height,
            tiles: candidate.widths.map((tileWidth, offset) => {
                const tile = {
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
function solve(values, width, count, relaxed, profile) {
    const costs = new Array(values.length + 1).fill(Infinity);
    const choices = new Array(values.length + 1).fill(null);
    costs[values.length] = 0;
    for (let start = values.length - 1; start >= 0; start--) {
        for (let end = start + 1; end <= Math.min(values.length, start + count); end++) {
            const ratios = values.slice(start, end);
            const available = width - (ratios.length - 1) * MODEL_WALL_GUTTER;
            const natural = available /
                Math.max(0.001, ratios.reduce((sum, value) => sum + value, 0));
            const final = end === values.length;
            const bounded = natural < profile.minimumHeight || natural > profile.maximumHeight;
            const candidates = [];
            if (!bounded || (!final && relaxed)) {
                const height = Math.min(profile.maximumHeight, Math.max(profile.minimumHeight, natural));
                const widths = ratios.map((ratio) => Math.max(1, ratio * height));
                const last = widths.length - 1;
                widths[last] = Math.max(1, (widths[last] ?? 1) + available - widths.reduce((sum, value) => sum + value, 0));
                const coverage = (widths.reduce((sum, value) => sum + value, 0) +
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
                const height = Math.min(profile.maximumHeight, Math.max(1, Math.min(profile.targetHeight, natural)));
                const widths = ratios.map((ratio) => Math.max(1, ratio * height));
                const coverage = (widths.reduce((sum, value) => sum + value, 0) +
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
    if (!Number.isFinite(costs[0]))
        return null;
    const rows = [];
    for (let index = 0; index < values.length;) {
        const choice = choices[index];
        if (!choice)
            return null;
        rows.push(choice);
        index = choice.end;
    }
    return rows;
}
/** Prefer full, readable trailing rows with Substitute's weighted comfort policy. */
function finalCost(height, coverage, count, width, ragged, profile) {
    const comfort = Math.max(profile.minimumHeight * 0.9, profile.targetHeight * 0.82);
    return (Math.abs(height - profile.targetHeight) +
        Math.max(0, (1 - coverage) * width) * 0.18 +
        Math.max(0, comfort - height) * 6.5 +
        Math.max(0, count - 7) * 65 +
        (ragged ? 18 : 0));
}
/** Limit mounted DOM to nearby rows while preserving the full scrollable wall height. */
export function visibleModelRows(rows, scrollTop, height) {
    const overscan = MODEL_PICKER_WALL_PROFILE.maximumHeight * 2;
    return rows.filter((row) => row.top + row.height >= scrollTop - overscan && row.top <= scrollTop + height + overscan);
}
