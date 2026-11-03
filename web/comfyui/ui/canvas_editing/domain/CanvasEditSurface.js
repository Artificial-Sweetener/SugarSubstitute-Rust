/**
 * Resolve the surface in the reference precedence.
 *
 * The exclusive coverage panel wins, then a selection modification preview, then a non-affine
 * edit session (or the armed shared-edges tool), then an affine transform, then the selection
 * actions. `modifying` is the page's own preview request, which may precede the host's session.
 */
export function canvasEditSurface(state, modifying) {
    const session = state.session;
    if (state.mode === 'coverage')
        return { kind: 'coverage' };
    if (modifying || session?.kind === 'modify')
        return { kind: 'modify' };
    if (session?.kind === 'move' || session?.kind === 'shared-edges')
        return { kind: 'session', tool: session.kind, armed: false };
    if (!session && state.tool === 'shared-edges')
        return { kind: 'session', tool: 'shared-edges', armed: true };
    if (session?.kind === 'transform')
        return { kind: 'transform', target: state.selection ? 'selection' : 'layer' };
    if (state.selection)
        return { kind: 'selection' };
    return { kind: 'none' };
}
/** Hide the rail and tool options while an exclusive editor owns canvas input. */
export function canvasEditChromeSuppressed(surface) {
    return surface.kind === 'coverage' || surface.kind === 'modify' || surface.kind === 'transform';
}
