import { resolveCubeFaceCardPresentation, } from './CubeFaceCardPolicy.js';
import { resolveCubeCardWidgetSources } from './CubeCardWidgetSources.js';
import { cubeFaceCardColumnSpan, findCubeFacePromptWidget } from './CubeFacePromptPolicy.js';
/** Keep exposed controls, prompt spans and reading order identical across both surfaces. */
export function projectNodeCards(nodes, state, graph, cube) {
    const sources = new Map(nodes.map((node) => [node, cube ? resolveCubeCardWidgetSources(cube, node) : []]));
    const additionalControlNodes = new Set(nodes.filter((node) => (sources.get(node)?.length ?? 0) > 0));
    return resolveCubeFaceCardPresentation(nodes, state, graph, additionalControlNodes).cards.map((card) => {
        const widgetSources = sources.get(card.node) ?? [];
        const exposed = new Set(widgetSources.map((source) => source.targetName));
        const presentedNode = {
            ...card.node,
            inputs: (card.node.inputs ?? []).filter((input) => !exposed.has(input.widget?.name ?? '')),
        };
        return {
            ...card,
            widgetSources,
            prompt: findCubeFacePromptWidget(presentedNode) !== null,
            columnSpan: cubeFaceCardColumnSpan(presentedNode),
        };
    });
}
