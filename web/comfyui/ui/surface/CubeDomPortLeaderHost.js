//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Maintain presentation-only DOM leaders between output labels and sockets. */
import { CUBE_OUTPUT_LEADER_LABEL_INSET, CUBE_PREVIEW_EDGE_INSET, resolveCubeOutputPortY, } from './CubeOutputLeaderGeometry.js';
const SVG_NAMESPACE = 'http://www.w3.org/2000/svg';
/** Own one reusable SVG layer without rebuilding paths during animation. */
export class CubeDomPortLeaderHost {
    #body;
    #svg;
    #paths = new Map();
    /** Mount one pointer-transparent leader layer into the native Cube body. */
    constructor(body) {
        this.#body = body;
        this.#svg = body.ownerDocument.createElementNS(SVG_NAMESPACE, 'svg');
        this.#svg.classList.add('sugarcubes-cube-port-leaders');
        this.#svg.dataset.sugarcubePortLeaders = '';
        this.#svg.setAttribute('aria-hidden', 'true');
        this.#svg.style.overflow = 'hidden';
        body.append(this.#svg);
    }
    /** Update exact orthogonal paths from measured titles and animated port Ys. */
    render(leaders) {
        if (this.#svg.parentElement !== this.#body)
            this.#body.append(this.#svg);
        const active = new Set(leaders.map((leader) => leader.index));
        for (const [index, path] of this.#paths) {
            if (active.has(index))
                continue;
            path.remove();
            this.#paths.delete(index);
        }
        const bodyRect = this.#body.getBoundingClientRect();
        const scale = this.#body.offsetWidth > 0 ? bodyRect.width / this.#body.offsetWidth : 1;
        const safeScale = Number.isFinite(scale) && scale > 0 ? scale : 1;
        const elbowX = Math.max(0, this.#body.offsetWidth - CUBE_PREVIEW_EDGE_INSET / 2);
        for (const leader of leaders) {
            const path = this.#paths.get(leader.index) ?? this.#createPath(leader.index);
            const label = leader.title.querySelector('[data-cube-preview-output-label]') ?? leader.title;
            const titleRect = label.getBoundingClientRect();
            const labelEndX = Math.min(elbowX, Math.max(0, (titleRect.right - bodyRect.left) / safeScale + CUBE_OUTPUT_LEADER_LABEL_INSET));
            const labelY = (titleRect.top + titleRect.height / 2 - bodyRect.top) / safeScale;
            const portY = resolveCubeOutputPortY(labelY, leader.portY);
            const socketRimX = Math.max(elbowX, this.#body.offsetWidth - leader.socketRadius);
            path.setAttribute('d', `M ${format(labelEndX)} ${format(labelY)} ` +
                `H ${format(elbowX)} V ${format(portY)} H ${format(socketRimX)}`);
        }
    }
    /** Remove the exact extension-owned SVG layer. */
    dispose() {
        this.#paths.clear();
        this.#svg.remove();
    }
    /** Create one stable path for a canonical output index. */
    #createPath(index) {
        const path = this.#body.ownerDocument.createElementNS(SVG_NAMESPACE, 'path');
        path.dataset.sugarcubeOutputLeader = String(index);
        path.setAttribute('fill', 'none');
        path.setAttribute('stroke', 'currentColor');
        path.setAttribute('stroke-opacity', '0.62');
        path.setAttribute('stroke-width', '1.25');
        path.setAttribute('stroke-linejoin', 'round');
        this.#svg.append(path);
        this.#paths.set(index, path);
        return path;
    }
}
/** Keep generated SVG path coordinates compact and finite. */
function format(value) {
    return (Number.isFinite(value) ? value : 0).toFixed(2);
}
