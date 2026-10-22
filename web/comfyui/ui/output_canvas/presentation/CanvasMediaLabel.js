/** Preserve still-image terminology while identifying the selected media's actual type. */
export function canvasMediaLabel(kind) {
    switch (kind) {
        case 'model':
            return '3D model';
        case 'video':
            return 'video';
        case 'animated':
            return 'animation';
        case 'image':
            return 'image';
        case 'unsupported':
            return 'output';
    }
}
