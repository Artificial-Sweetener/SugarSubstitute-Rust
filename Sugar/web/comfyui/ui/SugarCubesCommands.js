/**
 * Build the extension's `commands` and `menuCommands`.
 *
 * The project commands also sit in Comfy's File menu, so a page without SugarSubstitute's frame
 * offers them where it offers its own open and save.
 */
export function sugarCubesCommandRegistration(owners) {
    return {
        commands: [
            {
                id: 'SugarCubes.CubeStackCart',
                label: 'Add cubes / edit Cube stack',
                function: () => owners.openCart(),
            },
            {
                id: 'SugarCubes.ToggleMasonry',
                label: 'Toggle workflow masonry',
                function: () => owners.toggleMasonry(),
            },
            {
                id: 'SugarCubes.ToggleOutputCanvas',
                label: 'Toggle output canvas',
                function: () => owners.toggleCanvas(),
            },
            ...owners.projects.map((command) => ({
                id: command.id,
                label: command.label,
                function: () => command.function(),
            })),
        ],
        menuCommands: [{ path: ['File'], commands: owners.projects.map((command) => command.id) }],
    };
}
