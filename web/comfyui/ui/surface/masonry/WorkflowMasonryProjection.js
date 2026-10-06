import { requireCubeIdentity } from '../../cube/node/ComfyCubeNodeFactory.js';
import { orderWorkflowNodes } from '../WorkflowNodeOrder.js';
import { readCubeFaceCardState } from '../CubeFaceCardStateService.js';
import { projectNodeCards } from '../NodeCardProjection.js';
/** Keep real Cubes and consecutive loose nodes in distinct runs of the shared grid. */
export function projectWorkflowMasonry(graph, cubes, connections = []) {
    const cubeSet = new Set(cubes);
    const sections = [];
    const looseMembers = new Map();
    let loose = null;
    for (const node of orderWorkflowNodes(graph, connections)) {
        const id = String(node.id);
        const cube = cubeSet.has(node) ? cubes.find((candidate) => candidate === node) : undefined;
        if (cube) {
            loose = null;
            const identity = requireCubeIdentity(cube);
            sections.push({
                id: `cube:${String(identity.instance_id ?? cube.id)}`,
                label: cube.title?.trim() || 'SugarCube',
                cube,
                target: cube,
                cards: orderedCards(cube.subgraph._nodes, cube.subgraph, `cube:${String(cube.id)}`, cube, cube),
            });
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
function orderedCards(nodes, graph, prefix, target, cube) {
    return projectNodeCards(nodes, readCubeFaceCardState(target), graph, cube).map((card) => ({
        ...card,
        id: `${prefix}:${card.id}`,
        columnSpan: card.columnSpan ?? 1,
    }));
}
