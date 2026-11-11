const NUDGES = {
    ArrowLeft: [-1, 0],
    ArrowRight: [1, 0],
    ArrowUp: [0, -1],
    ArrowDown: [0, 1],
};
/**
 * Resolve one key to its editing intent, or null when the canvas does not own it.
 *
 * Space holds temporary navigation; Ctrl+Z, Ctrl+Shift+Z and Ctrl+Y walk document history;
 * Enter commits and Esc cancels; Delete clears the selected area; arrows nudge by one pixel, or
 * ten with Shift, while moving or transforming. The coverage panel blocks every key except Esc,
 * and a selection modification preview admits only navigation, Enter and Esc.
 */
export function canvasEditKeyIntent(key, state, surface) {
    if (surface.kind === 'coverage')
        return key.phase === 'down' && key.key === 'Escape' ? { kind: 'close-coverage' } : null;
    // Auto-repeat still restates the hold, so the page never scrolls; the owner sends changes only.
    if (key.key === ' ' && !key.ctrl && !key.meta && !key.alt)
        return { kind: 'command', command: { type: 'navigation', held: key.phase === 'down' } };
    if (key.phase !== 'down')
        return null;
    if (surface.kind === 'modify') {
        if (key.key === 'Escape')
            return { kind: 'cancel-modify' };
        return key.key === 'Enter' ? { kind: 'apply-modify' } : null;
    }
    const command = (key.ctrl || key.meta) && !key.alt ? historyCommand(key) : plainCommand(key, state);
    return command ? { kind: 'command', command } : null;
}
function historyCommand(key) {
    const letter = key.key.toLowerCase();
    if (letter === 'z')
        return { type: 'history', action: key.shift ? 'redo' : 'undo' };
    if (letter === 'y' && !key.shift)
        return { type: 'history', action: 'redo' };
    return null;
}
function plainCommand(key, state) {
    if (key.ctrl || key.meta || key.alt)
        return null;
    if (key.key === 'Enter')
        return { type: 'commit' };
    if (key.key === 'Escape')
        return { type: 'cancel' };
    if (key.key === 'Delete')
        return state.selection ? { type: 'selection', action: 'clear' } : null;
    const nudge = NUDGES[key.key];
    if (!nudge || !nudges(state))
        return null;
    const step = key.shift ? 10 : 1;
    return { type: 'nudge', dx: nudge[0] * step, dy: nudge[1] * step };
}
/** Nudge only where the host moves pixels: the Move tool and move or transform sessions. */
function nudges(state) {
    return (state.tool === 'move' || state.session?.kind === 'move' || state.session?.kind === 'transform');
}
