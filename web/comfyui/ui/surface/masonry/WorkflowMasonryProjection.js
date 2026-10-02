import { requireCubeIdentity } from '../../cube/node/ComfyCubeNodeFactory.js';
import { resolveCubeFaceCardPresentation, orderNodesUpstreamFirst, } from '../CubeFaceCardPolicy.js';
import { readCubeFaceCardState } from '../CubeFaceCardStateService.js';
import { cubeFaceCardColumnSpan } from '../CubeFacePromptPolicy.js';
import { resolveMasonryWidgetSources } from './WorkflowMasonryWidgetSources.js';
/** Keep real Cubes and consecutive loose nodes in distinct runs of the shared grid. */
export function projectWorkflowMasonry(graph, cubes, connections = []) {
    const nodes = [...(graph._nodes ?? graph.nodes ?? [])].sort((a, b) => (a.pos?.[0] ?? 0) - (b.pos?.[0] ?? 0) || (a.pos?.[1] ?? 0) - (b.pos?.[1] ?? 0));
    const byId = new Map(nodes.map((node) => [String(node.id), node]));
    const cubeSet = new Set(cubes);
    const sections = [];
    const looseMembers = new Map();
    let loose = null;
    for (const id of orderNodesUpstreamFirst(nodes, graph, connections)) {
        const node = byId.get(id);
        if (!node)
            continue;
        const cube = cubeSet.has(node) ? cubes.find((candidate) => candidate === node) : undefined;
        if (cube) {
            loose = null;
            const identity = requireCubeIdentity(cube);
            sections.push({
                id: `cube:${String(identity.instance_id ?? cube.id)}`,
                label: cube.title?.trim() || 'SugarCube',
                cube,
                target: cube,
                cards: orderedCards(cube.subgraph._nodes, cube.subgraph, `cube:${String(cube.id)}`, cube),
            });
            for (const card of sections[sections.length - 1]?.cards ?? []) {
                const exposed = new Set(resolveMasonryWidgetSources(cube, card.node).map((source) => source.targetName));
                card.columnSpan = cubeFaceCardColumnSpan({
                    ...card.node,
                    inputs: (card.node.inputs ?? []).filter((input) => !exposed.has(input.widget?.name ?? '')),
                });
            }
            continue;
        }
        if (!loose) {
            loose = { id: `loose:${id}`, label: 'Workflow nodes', cube: null, target: graph, cards: [] };
            sections.push(loose);
            looseMembers.set(loose.id, []);
        }
        looseMembers.get(loose.id)?.push(node);
    }
    for (const section of sections) {
        if (!section.cube)
            section.cards = orderedCards(looseMembers.get(section.id) ?? [], graph, 'root', graph);
    }
    return sections;
}
/** Project the exact Cube-face decisions with namespaced placement identities. */
function orderedCards(nodes, graph, prefix, target) {
    return resolveCubeFaceCardPresentation(nodes, readCubeFaceCardState(target), graph).cards.map((card) => ({
        ...card,
        id: `${prefix}:${card.id}`,
        columnSpan: card.columnSpan ?? 1,
    }));
}
