/**
 * Caption a Cube's output with the Cube's own name.
 *
 * A Cube with several outputs adds the output's name so its tabs differ. `ancestors` are the titles of the graphs the Cube sits in.
 */
export function cubeOutputCaption(facts, ancestors = []) {
    const outputName = facts.outputName || 'Output';
    return {
        label: facts.outputCount > 1 ? `${facts.name} \u00b7 ${outputName}` : facts.name,
        breadcrumb: [...ancestors, facts.title, outputName],
    };
}
/**
 * Caption every Cube output of a decoded workflow for the canvas.
 *
 * A workflow decoded from its saved form names a Cube's output by the output alone, which is
 * what a run's provenance needs; the canvas shows the Cube's name instead. Sources that are not
 * Cube outputs are returned as they are.
 */
export function presentCanvasSources(sources) {
    return Object.freeze(sources.map((source) => source.cube
        ? Object.freeze({
            ...source,
            ...cubeOutputCaption(source.cube, source.breadcrumb.slice(0, -1)),
        })
        : source));
}
