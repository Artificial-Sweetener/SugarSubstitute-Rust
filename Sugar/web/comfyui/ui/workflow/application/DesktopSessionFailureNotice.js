/** Title the notice for one failed desktop session operation. */
export function desktopSessionFailureTitle(operation) {
    if (operation === 'capture')
        return 'Workspace not saved';
    if (operation === 'materialize')
        return 'Workspace not restored';
    return 'Desktop session unavailable';
}
/**
 * Build the reporter the desktop session uses: each failure is logged with the work it
 * interrupted and shown under that work's own title.
 */
export function desktopSessionFailureReporter(logger, notify, describe) {
    return (error, operation) => {
        logger.error('SugarCubes desktop session failed', { error, operation });
        notify('error', desktopSessionFailureTitle(operation), describe(error));
    };
}
