/** Preserve exception context while keeping routine partial preset application non-blocking. */
export function editorActionFeedback(logger, feedback) {
    return {
        reportError: (error) => {
            logger.error('SugarCubes editor action failed', { error });
            feedback.pushToast('error', 'Editor action unavailable', feedback.readErrorMessage(error));
        },
        skipped: (names) => logger.warn('SugarCubes preset skipped incompatible or connected fields', { fields: names }),
    };
}
