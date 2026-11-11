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
/**
 * Paint the badge and action-pill content of one Cube chrome header.
 */
import { clampNumber, resolveGroupTitleColor, } from './CubeChromeLayout.js';
/** Paint computed badge and action-pill content. */
export class CubeChromeHeaderContentPainter {
    painter;
    constructor(painter) {
        this.painter = painter;
    }
    paintBadge({ ctx, layout, slot, centerY, badgeHeight, paddingY, lineGap, nameSize, fontFamily, baseAlpha, group, instanceId, fontSize, hover, regions, }) {
        if (!layout.visible)
            return;
        const badgeX = clampNumber(slot.center - layout.width / 2, slot.left, slot.right - layout.width);
        const badgeY = centerY - badgeHeight / 2;
        const nameLine = layout.lines.find((line) => line.key === 'name');
        const authorLine = layout.lines.find((line) => line.key === 'author');
        const titleColor = resolveGroupTitleColor(group);
        const textCenterX = badgeX + layout.width / 2;
        if (nameLine?.text) {
            ctx.save();
            ctx.globalAlpha = baseAlpha * 0.95;
            ctx.font = `${nameLine.size}px ${fontFamily}`;
            ctx.fillStyle = titleColor;
            ctx.textAlign = 'center';
            ctx.fillText(nameLine.text, textCenterX, badgeY + paddingY + nameLine.size / 2);
            ctx.restore();
            regions.push({
                key: 'name',
                instanceId,
                rect: {
                    x: badgeX,
                    y: badgeY + paddingY,
                    w: layout.width,
                    h: nameLine.size + lineGap / 2,
                },
                truncated: nameLine.truncated,
                fullText: nameLine.fullText,
            });
            if (nameLine.truncated &&
                hover.hoveredBadgeInstance === instanceId &&
                hover.hoveredBadgeKey === 'name') {
                this.painter.drawTooltip(ctx, textCenterX, badgeY, nameLine.fullText, fontSize);
            }
        }
        if (authorLine?.text) {
            ctx.save();
            ctx.globalAlpha = baseAlpha * 0.75;
            ctx.font = `${authorLine.size}px ${fontFamily}`;
            ctx.fillStyle = titleColor;
            ctx.textAlign = 'center';
            ctx.fillText(authorLine.text, textCenterX, badgeY + paddingY + nameSize + lineGap + authorLine.size / 2);
            ctx.restore();
            regions.push({
                key: 'author',
                instanceId,
                rect: {
                    x: badgeX,
                    y: badgeY + paddingY + nameSize + lineGap / 2,
                    w: layout.width,
                    h: authorLine.size + lineGap / 2,
                },
                truncated: authorLine.truncated,
                fullText: authorLine.fullText,
            });
            if (authorLine.truncated &&
                hover.hoveredBadgeInstance === instanceId &&
                hover.hoveredBadgeKey === 'author') {
                this.painter.drawTooltip(ctx, textCenterX, badgeY, authorLine.fullText, fontSize);
            }
        }
    }
    paintPills({ ctx, entries, cursorX, centerY, pillHeight, gap, instanceId, now, fontSize, metadata, hover, regions, }) {
        for (const { item, pillWidth } of entries) {
            const isHovered = hover.hoveredKey === item.key && (!instanceId || hover.hoveredInstance === instanceId);
            const pillX = cursorX;
            const pillY = centerY - pillHeight / 2 - (isHovered ? 1 : 0);
            this.painter.drawPill(ctx, pillX, pillY, pillWidth, pillHeight, item.style, {
                hovered: isHovered,
                pulse: 0,
                alpha: 1,
            });
            if (item.icon) {
                const iconSize = Math.max(12, fontSize - 2);
                this.painter.drawIcon(ctx, item.icon, pillX + (pillWidth - iconSize) / 2, centerY - iconSize / 2, iconSize, item.color, { now, alpha: 1 });
            }
            else if (item.label) {
                const align = item.labelAlign === 'center' ? 'center' : 'left';
                const labelSize = Number(item.labelScale) && Number.isFinite(item.labelScale)
                    ? Math.max(10, Math.round(fontSize * Number(item.labelScale)))
                    : fontSize;
                ctx.save();
                ctx.fillStyle = item.color || '#f3f7fb';
                ctx.font = `${labelSize}px sans-serif`;
                ctx.textAlign = align;
                ctx.fillText(item.label, align === 'center' ? pillX + pillWidth / 2 : pillX + 8, centerY);
                ctx.restore();
            }
            if (item.style === 'action') {
                regions.push({
                    key: item.key,
                    tooltip: item.tooltip || '',
                    instanceId,
                    metadata,
                    flavorOptions: item.flavorOptions || null,
                    rect: { x: pillX, y: pillY, w: pillWidth, h: pillHeight },
                });
            }
            if (isHovered && item.tooltip) {
                this.painter.drawTooltip(ctx, pillX + pillWidth / 2, pillY, item.tooltip, fontSize);
            }
            cursorX += pillWidth + gap;
        }
    }
}
