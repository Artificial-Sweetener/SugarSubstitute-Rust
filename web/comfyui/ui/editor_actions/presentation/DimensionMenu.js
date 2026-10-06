import { LANDSCAPE_RATIOS, PORTRAIT_RATIOS, ratioDimension, canonicalDimensions, } from '../domain/DimensionPolicy.js';
import { GLOBAL_ASSOCIATION } from '../domain/UserPreset.js';
/** Build side-aware dimensions, ratio orientations and matching persisted scopes without IO. */
export function dimensionMenuEntries(binding, side, library, families, run) {
    if (binding.width.disabled() || binding.height.disabled())
        return [];
    const entries = [
        {
            label: 'Swap width & height',
            select: () => {
                const width = binding.width.read(), height = binding.height.read();
                if (typeof width === 'number' && typeof height === 'number') {
                    binding.width.write(height);
                    binding.height.write(width);
                }
            },
        },
    ];
    const scopes = [...families, GLOBAL_ASSOCIATION];
    if (scopes.some((scope) => library.dimensions(scope).length))
        entries.push({
            label: 'Set dimensions',
            children: [false, true].map((landscape) => ({
                label: landscape ? 'Landscape' : 'Portrait',
                children: scopes
                    .map((scope) => ({ scope, presets: library.dimensions(scope) }))
                    .filter(({ presets }) => presets.length)
                    .flatMap(({ scope, presets }, index) => [
                    ...(index > 0 ? [{ label: '', separator: true }] : []),
                    {
                        label: scope.scope === 'global' ? 'Global' : `For ${scope.label}`,
                        section: true,
                    },
                    ...presets.map((preset) => {
                        const width = landscape ? preset.payload.long_edge : preset.payload.short_edge;
                        const height = landscape ? preset.payload.short_edge : preset.payload.long_edge;
                        const dimensions = `${width} x ${height}`;
                        const label = [
                            dimensions,
                            `${preset.payload.short_edge} x ${preset.payload.long_edge}`,
                        ].includes(preset.label.trim())
                            ? dimensions
                            : `${preset.label} ${dimensions}`;
                        return {
                            label,
                            select: () => {
                                binding.width.write(width);
                                binding.height.write(height);
                            },
                        };
                    }),
                ]),
            })),
        });
    for (const anchor of side ? [side] : ['width', 'height'])
        entries.push({
            label: `Set ratio by ${anchor === 'width' ? 'Width' : 'Height'}`,
            children: [
                { label: 'Landscape', ratios: LANDSCAPE_RATIOS },
                { label: 'Portrait', ratios: PORTRAIT_RATIOS },
            ].map(({ label, ratios }) => ({
                label,
                children: ratios.map(([width, height]) => ({
                    label: `${width}:${height}`,
                    select: () => {
                        const value = binding[anchor].read();
                        const target = typeof value === 'number' ? ratioDimension(value, anchor, width, height) : null;
                        if (target !== null)
                            binding[anchor === 'width' ? 'height' : 'width'].write(target);
                    },
                })),
            })),
        });
    const width = binding.width.read(), height = binding.height.read();
    if (typeof width === 'number' && typeof height === 'number' && canonicalDimensions(width, height))
        entries.push({ label: '', separator: true }, {
            label: 'Save current dimensions',
            children: [
                {
                    label: 'Save globally',
                    select: () => run(library.saveDimensions(width, height, GLOBAL_ASSOCIATION)),
                },
                ...families.slice(0, 1).map((scope) => ({
                    label: `Save for ${scope.label}`,
                    select: () => run(library.saveDimensions(width, height, scope)),
                })),
            ],
        });
    return entries;
}
