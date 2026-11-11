import { requireCubeIdentity } from '../cube/node/ComfyCubeNodeFactory.js';
import { resolveCubeFaceCardPresentation, } from './CubeFaceCardPolicy.js';
import { presentCubeCardNode, resolveCubeCardWidgetSources } from './CubeCardWidgetSources.js';
import { cubeFaceCardColumnSpan, findCubeFacePromptWidget } from './CubeFacePromptPolicy.js';
/** Keep exposed controls, prompt spans and reading order identical across both surfaces. */
export function projectNodeCards(nodes, state, graph, cube) {
    const sources = new Map(nodes.map((node) => [node, cube ? resolveCubeCardWidgetSources(cube, node) : []]));
    const additionalControls = new Map(nodes
        .filter((node) => (sources.get(node)?.length ?? 0) > 0)
        .map((node) => [
        node,
        (sources.get(node) ?? []).flatMap((source) => {
            const widget = source.node.widgets?.find((widget) => widget.name === source.name);
            return widget ? [widget] : [];
        }),
    ]));
    return resolveCubeFaceCardPresentation(nodes, state, graph, additionalControls).cards.map((card) => {
        const widgetSources = sources.get(card.node) ?? [];
        const presentedNode = presentCubeCardNode(card.node, widgetSources);
        return {
            ...card,
            ...(cube ? { cubeId: String(requireCubeIdentity(cube).instance_id ?? cube.id) } : {}),
            widgetSources,
            prompt: findCubeFacePromptWidget(presentedNode) !== null,
            columnSpan: cubeFaceCardColumnSpan(presentedNode),
        };
    });
}
