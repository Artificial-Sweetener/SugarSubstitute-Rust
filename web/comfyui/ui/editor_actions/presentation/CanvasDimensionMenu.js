import { CanvasDimensionScaleMenu, } from './CanvasDimensionScaleMenu.js';
/** Describe commands without introducing controls, style overrides or another menu implementation. */
export function canvasDimensionMenuEntries(available, execute, run) {
    if (!available)
        return [];
    const actions = [
        ['generation-from-canvas', 'Set size from canvas'],
        ['canvas-from-generation', 'Resize canvas to size'],
        ['generation-from-selection', 'Set size from selection'],
        ['generation-from-canvas-scaled', 'Set size from canvas with scaling'],
        ['canvas-from-generation-scaled', 'Resize canvas to size with scaling'],
        ['generation-from-selection-scaled', 'Set size from selection with scaling'],
    ];
    return [
        { label: '', separator: true },
        ...actions.map(([action, label]) => {
            if (action.endsWith('-scaled')) {
                const menu = new CanvasDimensionScaleMenu(action, execute, run);
                return { label, children: menu.entries(), opened: menu.opened };
            }
            return {
                label,
                select: () => {
                    run(execute(action));
                },
            };
        }),
    ];
}
