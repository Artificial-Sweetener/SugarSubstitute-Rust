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
/** Capture native link strokes once and suspend only their paint during presentation travel. */
import { isRecord } from '../../types/common.js';
import { NativeLinkPathRecording, } from './NativeLinkPathRecording.js';
/** Resolve an optional native renderer method at the dynamic Comfy boundary. */
function method(owner, name) {
    const value = owner[name];
    return typeof value === 'function'
        ? (...arguments_) => Reflect.apply(value, owner, arguments_)
        : null;
}
/** Capture existing visible routes, preserving canonical paths and all unrelated canvas painting. */
export function captureMasonryLinkPaths(canvas, container) {
    if (!canvas || !isRecord(canvas.linkRenderer) || !isRecord(canvas.linkRenderer.pathRenderer))
        return null;
    const renderer = canvas.linkRenderer.pathRenderer;
    const draw = method(canvas, 'draw');
    const drawPath = method(renderer, 'drawLinkPath');
    const drawLink = method(renderer, 'drawLink');
    const build = new Map([
        ['spline', method(renderer, 'buildSplinePath')],
        ['straight', method(renderer, 'buildStraightPath')],
        ['linear', method(renderer, 'buildLinearPath')],
    ]);
    if (!draw || !drawPath || [...build.values()].some((builder) => !builder))
        return null;
    const original = Object.getOwnPropertyDescriptor(renderer, 'drawLinkPath');
    const originalLink = Object.getOwnPropertyDescriptor(renderer, 'drawLink');
    const strokes = [];
    const bounds = container.getBoundingClientRect();
    const hostBounds = canvas.canvas?.getBoundingClientRect();
    let collecting = true;
    let overflow = false;
    let restored = false;
    const replacement = (...arguments_) => {
        if (restored)
            return drawPath(...arguments_);
        const [context, , link, options, width, color] = arguments_;
        if (!(context instanceof CanvasRenderingContext2D) ||
            !isRecord(link) ||
            !isRecord(options) ||
            !isRecord(options.style) ||
            typeof width !== 'number' ||
            typeof color !== 'string' ||
            typeof link.id !== 'string' ||
            link.id === 'temp')
            return drawPath(...arguments_);
        const builder = typeof options.style.mode === 'string' ? build.get(options.style.mode) : null;
        if (!builder)
            return drawPath(...arguments_);
        if (collecting) {
            if (strokes.length >= 512)
                overflow = true;
            else {
                const recording = new NativeLinkPathRecording();
                builder(recording, link.startPoint, link.endPoint, link.startDirection, link.endDirection, link.controlPoints);
                const transform = context.getTransform();
                // Comfy blits its background buffer divided by DPR, undoing the buffer's paint scale.
                const backgroundScale = context.canvas === canvas.bgcanvas && canvas.bgcanvas !== canvas.canvas
                    ? container.ownerDocument.defaultView?.devicePixelRatio || 1
                    : 1;
                const paintScaleX = hostBounds
                    ? hostBounds.width / context.canvas.width / backgroundScale
                    : 1;
                const paintScaleY = hostBounds
                    ? hostBounds.height / context.canvas.height / backgroundScale
                    : 1;
                strokes.push({
                    link: link.id,
                    path: recording.path,
                    color,
                    width,
                    transform: `matrix(${transform.a * paintScaleX} ${transform.b * paintScaleY} ${transform.c * paintScaleX} ${transform.d * paintScaleY} ${transform.e * paintScaleX + (hostBounds?.x ?? bounds.x) - bounds.x} ${transform.f * paintScaleY + (hostBounds?.y ?? bounds.y) - bounds.y})`,
                    opacity: context.globalAlpha,
                    lineCap: context.lineCap,
                });
            }
        }
        const ownStroke = Object.getOwnPropertyDescriptor(context, 'stroke');
        Object.defineProperty(context, 'stroke', { configurable: true, value: () => undefined });
        try {
            return drawPath(...arguments_);
        }
        finally {
            if (ownStroke)
                Object.defineProperty(context, 'stroke', ownStroke);
            else
                Reflect.deleteProperty(context, 'stroke');
        }
    };
    /** Keep markers and execution-flow decoration from remaining after their stroke retracts. */
    const linkReplacement = (...arguments_) => {
        if (!drawLink)
            return undefined;
        const [context, link, options] = arguments_;
        if (restored || !isRecord(options) || !isRecord(options.style))
            return drawLink(...arguments_);
        return drawLink(context, link, {
            ...options,
            animation: undefined,
            style: { ...options.style, showArrows: false, showCenterMarker: false },
        });
    };
    /** Restore exactly the native hook that existed before this presentation. */
    const restore = () => {
        if (restored)
            return;
        restored = true;
        if (renderer.drawLinkPath === replacement) {
            if (original)
                Object.defineProperty(renderer, 'drawLinkPath', original);
            else
                Reflect.deleteProperty(renderer, 'drawLinkPath');
        }
        if (renderer.drawLink === linkReplacement) {
            if (originalLink)
                Object.defineProperty(renderer, 'drawLink', originalLink);
            else
                Reflect.deleteProperty(renderer, 'drawLink');
        }
        draw(false, true);
    };
    Object.defineProperty(renderer, 'drawLinkPath', {
        configurable: true,
        writable: true,
        value: replacement,
    });
    if (drawLink)
        Object.defineProperty(renderer, 'drawLink', {
            configurable: true,
            writable: true,
            value: linkReplacement,
        });
    try {
        draw(false, true);
        collecting = false;
        if (!strokes.length || overflow) {
            restore();
            return null;
        }
        return { strokes, restore };
    }
    catch (error) {
        restore();
        throw error;
    }
}
