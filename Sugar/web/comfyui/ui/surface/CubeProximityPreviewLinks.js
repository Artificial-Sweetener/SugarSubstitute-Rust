/** Preserve the existing preview boundary by excluding disabled proximity and incomplete endpoints. */
export function projectCubeProximityPreviewLinks(enabled, matches) {
    return enabled
        ? matches.flatMap((match) => match.outputId === undefined || match.inputId === undefined
            ? []
            : [{ originId: match.outputId, originSlot: match.outputSlot, targetId: match.inputId }])
        : [];
}
