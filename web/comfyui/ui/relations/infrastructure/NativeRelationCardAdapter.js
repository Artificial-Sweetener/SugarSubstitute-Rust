import { isRecord } from '../../types/common.js';
import { nativeNodeFieldRows } from '../../surface/NativeNodeFieldRows.js';
import { CUBE_FACE_PROJECTION_SYMBOL } from '../../surface/ComfyLiteGraphCubeFaceProjectionHost.js';
import { cubeFacePresentedWidgets } from '../../surface/CubeFaceWidgetPolicy.js';
/** Resolve already-mounted views of one eligible face node in either native renderer. */
export function nativeRelationCards(document, node, cubeId) {
    const locator = `${String(node.graph?.id)}:${String(node.id)}`;
    const cells = [
        ...document.querySelectorAll('.sugarcubes-cube-face__node-card'),
    ].filter((cell) => cell.dataset.cubeNodeLocator === locator);
    const roots = cubeId !== null
        ? cells
        : [
            ...cells,
            ...[...document.querySelectorAll('.lg-node[data-node-id]')].filter((root) => root.dataset.nodeId === String(node.id)),
        ];
    const projection = Reflect.get(node, CUBE_FACE_PROJECTION_SYMBOL);
    if (isRecord(projection) &&
        projection.container instanceof HTMLElement &&
        typeof projection.projectRect === 'function') {
        const projectRect = projection.projectRect;
        const root = projection.container;
        const width = Number.parseFloat(root.style.width);
        const bounds = root.getBoundingClientRect();
        const scale = bounds.width / width;
        const fields = canvasFields(node, width, (y) => {
            const rect = projectRect.call(projection, [0, y, 1, 1]);
            return isRecord(rect) && typeof rect.top === 'number' && scale > 0
                ? (rect.top - bounds.top) / scale
                : y;
        });
        return [
            {
                root,
                header: null,
                rows: new Map(),
                canvas: null,
                canvasFields: fields,
                canvasTitle: true,
            },
        ];
    }
    return roots.map((root) => {
        const card = root.matches('.lg-node') ? root : root.querySelector('.lg-node');
        const canvas = root.querySelector('canvas[data-cube-face-native="nodes-1"]');
        const header = card?.querySelector('.lg-node-header') ?? null;
        const rows = nativeNodeFieldRows(root);
        const aspect = canvas?.style.aspectRatio.split('/').map((value) => Number.parseFloat(value));
        const width = aspect?.[0] ?? (canvas ? root.clientWidth : 0);
        const height = aspect?.[1] ?? (canvas ? root.clientHeight : 0);
        const ratio = canvas && width > 0 ? canvas.getBoundingClientRect().width / width : 1;
        const canvasTop = canvas
            ? canvas.getBoundingClientRect().top - root.getBoundingClientRect().top
            : 0;
        const fields = canvas
            ? canvasFields(node, width, (y) => canvasTop + (30 + y) * ratio, ratio)
            : new Map();
        return {
            root,
            header,
            rows,
            canvas,
            canvasFields: fields,
            canvasTitle: canvas !== null && height > 0,
        };
    });
}
/** Read actual arranged LiteGraph widget positions for native DOM selectors on canvas faces. */
function canvasFields(node, width, top, scale = 1) {
    const fields = new Map();
    for (const widget of cubeFacePresentedWidgets(node)) {
        const y = typeof widget.y === 'number' ? widget.y : widget.last_y;
        if (typeof y !== 'number' || !Number.isFinite(y))
            continue;
        fields.set(widget.name, {
            top: top(y),
            left: 12 * scale,
            width: Math.max(1, width - 24) * scale,
            height: 24 * scale,
        });
    }
    return fields;
}
