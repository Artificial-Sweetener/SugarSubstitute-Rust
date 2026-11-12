/** Reuse execution acceptance when exposing implicit dependencies to workflow features. */
export function proximityWorkflowConnections(proximity, graph) {
    return proximity
        .resolveExecutionMatches(graph)
        .flatMap((match) => match.outputId === undefined || match.inputId === undefined
        ? []
        : [{ source: match.outputId, target: match.inputId }]);
}
