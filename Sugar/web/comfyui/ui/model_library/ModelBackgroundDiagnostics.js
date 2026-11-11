/** Preserve the original failure for diagnosis while keeping background work out of dialogs. */
export function modelBackgroundReporter(logger, operation) {
    return (error) => logger.warn('SugarCubes model background request failed', { operation, error });
}
