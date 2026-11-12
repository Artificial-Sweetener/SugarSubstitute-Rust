// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Map a participant whose pivot joined two corners, as CuteCanvas's shared-edge collapse does:
//! a quadrilateral maps bilinearly onto its collapsed self, a larger boundary is first reduced
//! by dropping vertices strictly between their neighbours.
use super::edges::{Point, cross, dot, sub};
use crate::CanvasError;
use sugarsubstitute_image_ops::{BilinearMapping, CoverageMapping, PiecewiseMapping};

/// CuteCanvas's mapping for a boundary whose moved corner joined a neighbour, or `None` when
/// no edge collapsed.
pub(super) fn joined_mapping(
    source: &[Point],
    target: &[Point],
    moved: usize,
) -> Result<Option<CoverageMapping>, CanvasError> {
    let count = target.len();
    let joined: Vec<usize> =
        (0..count).filter(|index| target[*index] == target[(index + 1) % count]).collect();
    let collapsed = match joined.as_slice() {
        [] => return Ok(None),
        [only] => *only,
        _ => return Err(CanvasError::Invalid("A pivot may join only one participant edge")),
    };
    let quad = |points: &[Point], start: usize| -> [Point; 4] {
        [0, 1, 2, 3].map(|offset| points[(start + offset) % 4])
    };
    if source.len() == 4 && count == 4 {
        return Ok(Some(CoverageMapping::Bilinear(BilinearMapping::new(
            quad(source, collapsed),
            quad(target, collapsed),
        )?)));
    }
    let pair = [collapsed, (collapsed + 1) % count];
    if !pair.contains(&moved) {
        return Err(CanvasError::Invalid("A joined mapping must name the moved corner"));
    }
    let source = simplified((0..source.len()).map(|index| (index, source[index])).collect(), 4);
    if source.len() != 4 {
        return Err(CanvasError::Invalid("A joined source must reduce to four corners"));
    }
    let target = simplified(
        (0..count).filter(|index| *index != moved).map(|index| (index, target[index])).collect(),
        3,
    );
    match target.len() {
        3 => {
            let start = source
                .iter()
                .position(|(index, _)| *index == collapsed)
                .ok_or(CanvasError::Invalid("Missing joined corner"))?;
            let source = [0, 1, 2, 3].map(|offset| source[(start + offset) % 4].1);
            let retained = pair.into_iter().find(|index| *index != moved).unwrap_or(collapsed);
            let at = target
                .iter()
                .position(|(index, _)| *index == retained)
                .ok_or(CanvasError::Invalid("Missing retained corner"))?;
            let apex = target[at].1;
            let quad = [apex, apex, target[(at + 1) % 3].1, target[(at + 2) % 3].1];
            Ok(Some(CoverageMapping::Bilinear(BilinearMapping::new(source, quad)?)))
        }
        4 => {
            let start = (0..4)
                .max_by_key(|position| {
                    // Python's `max` keeps the first best start; reverse ties to match.
                    let matches = (0..4)
                        .filter(|offset| source[(position + offset) % 4].0 == target[*offset].0)
                        .count();
                    (matches, std::cmp::Reverse(*position))
                })
                .unwrap_or(0);
            let source: Vec<Point> = (0..4).map(|offset| source[(start + offset) % 4].1).collect();
            let target: Vec<Point> = target.iter().map(|(_, point)| *point).collect();
            Ok(Some(CoverageMapping::Piecewise(PiecewiseMapping::new(&source, &target)?)))
        }
        _ => Err(CanvasError::Invalid("A joined target must reduce to three or four corners")),
    }
}

/// Remove the first vertex strictly between its neighbours on their line, repeatedly, while
/// more than `minimum` remain.
fn simplified(mut vertices: Vec<(usize, Point)>, minimum: usize) -> Vec<(usize, Point)> {
    while vertices.len() > minimum {
        let count = vertices.len();
        let removable = (0..count).find(|position| {
            let previous = vertices[(position + count - 1) % count].1;
            let point = vertices[*position].1;
            let following = vertices[(position + 1) % count].1;
            let edge = sub(following, previous);
            let length = dot(edge, edge);
            if length <= 1e-18 {
                return false;
            }
            let relative = sub(point, previous);
            if cross(edge, relative).abs() > 1e-9 * length.max(1.0) {
                return false;
            }
            let projection = dot(relative, edge) / length;
            0.0 < projection && projection < 1.0
        });
        match removable {
            Some(position) => {
                vertices.remove(position);
            }
            None => break,
        }
    }
    vertices
}
