import { isRecord } from '../../types/common.js';
import { findVueComponents, findMountedNodeVNode } from '../../surface/ComfyVueTree.js';
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
        const rows = new Map();
        const vnodeRoot = root.querySelector('.sugarcubes-cube-face__native-card-mount') ?? card;
        const localVNode = vnodeRoot ? Reflect.get(vnodeRoot, '_vnode') : null;
        const appRoot = document.getElementById('vue-app');
        const appVNode = appRoot ? Reflect.get(appRoot, '_vnode') : null;
        const vnode = localVNode ??
            (card ? findMountedNodeVNode(card) : null) ??
            (card
                ? findVueComponents(appVNode, 'LGraphNode').find((candidate) => {
                    const component = isRecord(candidate.component) ? candidate.component : {};
                    const subtree = isRecord(component.subTree) ? component.subTree : {};
                    return subtree.el === card;
                })
                : undefined);
        if (vnode)
            for (const container of findVueComponents(vnode, 'NodeWidgets')) {
                const component = isRecord(container.component) ? container.component : {}, tree = isRecord(component.subTree) ? component.subTree : {};
                if (!(tree.el instanceof HTMLElement))
                    continue;
                for (const child of findVueComponents(container, [
                    'WidgetSelect',
                    'WidgetInputNumber',
                    'WidgetInputNumberSlider',
                    'WidgetInputText',
                    'WidgetTextarea',
                    'WidgetToggle',
                ])) {
                    const instance = isRecord(child.component) ? child.component : {}, props = isRecord(instance.props) ? instance.props : {}, widget = isRecord(props.widget) ? props.widget : {}, subtree = isRecord(instance.subTree) ? instance.subTree : {};
                    if (!(subtree.el instanceof HTMLElement) || typeof widget.name !== 'string')
                        continue;
                    let row = subtree.el;
                    while (row.parentElement && row.parentElement !== tree.el)
                        row = row.parentElement;
                    if (row.parentElement === tree.el)
                        rows.set(widget.name, row);
                }
            }
        if (card)
            for (const row of card.querySelectorAll('[data-sc-fluent-row]')) {
                if (row.closest('.lg-node') !== card)
                    continue;
                const label = row
                    .querySelector('[data-testid="widget-layout-field-label"]')
                    ?.textContent?.trim();
                const control = row.querySelector('[role="combobox"][aria-label], [aria-label]');
                const name = control?.getAttribute('aria-label');
                if (name)
                    rows.set(name, row);
                else if (label)
                    rows.set(label, row);
            }
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
