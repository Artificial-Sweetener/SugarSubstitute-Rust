import { isCubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import { locateModelGraph } from './ComfyModelGraph.js';
import { assignCubeModelRequirement, cubeModelRequirement } from './CubeModelRequirements.js';
import { isScopedModelKind } from './ModelPickerCompatibilityScope.js';
/** Offer one portable scope owner only on Cube-face masonry, preserving loose node behavior. */
export function modelScopeMenu(app, binding, snapshot, changed) {
    if (!isScopedModelKind(binding.kind) ||
        !binding.anchor.closest('.sugarcubes-cube-face') ||
        !isCubeNode(locateModelGraph(app, binding)?.scope.owner))
        return [];
    const current = cubeModelRequirement(app, binding);
    const families = snapshot.relationships?.families.filter((family) => family.architecture) ?? [];
    const architectures = [
        ...new Set([
            ...families.map((family) => family.architecture),
            ...snapshot.items.flatMap((item) => (item.architecture ? [item.architecture.base] : [])),
        ]),
    ].sort();
    /** Apply policy before notifying the field owner to resolve installed selections. */
    const assign = (requirement) => {
        assignCubeModelRequirement(app, binding, requirement);
        changed();
    };
    return [
        {
            label: 'Cube models',
            children: [
                {
                    label: 'Unfiltered',
                    checked: current?.architecture === '' && current.family === '',
                    select: () => assign({ architecture: '', family: '' }),
                },
                ...architectures.map((architecture) => ({
                    label: architecture,
                    children: [
                        {
                            label: `All ${architecture} models`,
                            checked: current?.architecture === architecture && !current.family,
                            select: () => assign({ architecture, family: '' }),
                        },
                        ...families
                            .filter((family) => family.architecture === architecture)
                            .map((family) => ({
                            label: family.name,
                            checked: current?.family === family.key,
                            select: () => assign({ architecture, family: family.key }),
                        })),
                    ],
                })),
            ],
        },
    ];
}
