// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Present a page-laid-out overview grid as one composed native document.
//!
//! SugarCubes owns the grid layout, scrolling and cell clicks; the host only fits each visible
//! cell's output inside the physical rectangle the page reported for it.

#[cfg(test)]
use super::operations::fixture_file;
use super::{operations::admitted_output, *};
use crate::native_canvas::session::OverviewSelection;
#[cfg(test)]
use crate::native_canvas::session::SelectedOriginals;
use sugarsubstitute_contracts::canvas_surface::CanvasFileReference;
use sugarsubstitute_image_ops::Extent;

/// One visible overview cell: its output and CSS-pixel `[x, y, width, height]` rectangle,
/// relative to the surface geometry origin.
pub(super) struct OverviewCell {
    pub(super) file: CanvasFileReference,
    pub(super) rect: [f64; 4],
}

impl NativeSurface {
    /// Load every visible cell as one selection and answer only after its frame is presented.
    pub(super) fn overview(
        &mut self,
        identity: MainIdentity,
        request: Box<CanvasSurfaceRequest>,
        claim: MemoryClaim,
        cells: Vec<OverviewCell>,
        session: &mut Session,
    ) -> Result<(), Box<dyn Error>> {
        // A hidden candidate has no placement yet; the session's surface is the prepared extent.
        let density = self
            .binding
            .as_ref()
            .and_then(|binding| binding.allocation)
            .map(|allocation| allocation.physical_pixels_per_css_pixel);
        let Some(density) = density else {
            return self.reply(identity, &request, CanvasSurfaceReplyStatus::Unavailable, claim);
        };
        let viewport = session.surface_extent();
        let Some(selection) =
            layout(&cells, viewport.width(), viewport.height(), density, |cell| {
                admitted_output(&cell.file)
            })?
        else {
            return self.reply(identity, &request, CanvasSurfaceReplyStatus::Error, claim);
        };
        self.fence_previews(session)?;
        #[cfg(test)]
        if let Some(output) = &self.fixture_output {
            let Some(files) =
                layout(&cells, viewport.width(), viewport.height(), density, |cell| {
                    fixture_file(output, &cell.file)
                })?
            else {
                return self.reply(identity, &request, CanvasSurfaceReplyStatus::Error, claim);
            };
            if let Some(acquisition) = &self.fixture_acquisition {
                if !Self::offer_acquired(acquisition(files), session)? {
                    return self.reply(
                        identity,
                        &request,
                        CanvasSurfaceReplyStatus::Unavailable,
                        claim,
                    );
                }
                return self.await_selection(identity, request, claim, true, session);
            }
            let OverviewSelection { extent, primary: (primary, primary_cell), cells } = files;
            let mut originals = Some(SelectedOriginals {
                primary: ImageSource::file(primary),
                compared: None,
                overview: Some(crate::native_canvas::session::OverviewOriginals {
                    extent,
                    primary_cell,
                    cells: cells
                        .into_iter()
                        .map(|(path, cell)| (ImageSource::file(path), cell))
                        .collect(),
                }),
            });
            if !session.offer_originals(&mut originals, CacheReuse::Compatible, None)? {
                return self.reply(
                    identity,
                    &request,
                    CanvasSurfaceReplyStatus::Unavailable,
                    claim,
                );
            }
            return self.await_selection(identity, request, claim, true, session);
        }
        if !self.offer_comfy_overview(identity, selection, session)? {
            return self.reply(identity, &request, CanvasSurfaceReplyStatus::Unavailable, claim);
        }
        self.await_selection(identity, request, claim, true, session)
    }

    /// Whether the presented selection is an overview, whose camera cannot be navigated.
    pub(super) fn showing_overview(&self) -> bool {
        self.binding.as_ref().is_some_and(|binding| binding.overview)
    }
}

/// Resolve each cell's identity and convert its CSS rectangle into physical document pixels.
/// Returns `None` when any cell's output is not admissible.
fn layout<T>(
    cells: &[OverviewCell],
    width: u32,
    height: u32,
    density: f64,
    resolve: impl Fn(&OverviewCell) -> Option<T>,
) -> Result<Option<OverviewSelection<T>>, Box<dyn Error>> {
    let extent = Extent::new(width, height)?;
    let mut resolved = Vec::with_capacity(cells.len());
    for cell in cells {
        let Some(identity) = resolve(cell) else { return Ok(None) };
        let [x, y, cell_width, cell_height] = cell.rect;
        resolved.push((
            identity,
            [x * density, y * density, cell_width * density, cell_height * density],
        ));
    }
    let mut resolved = resolved.into_iter();
    let primary = resolved.next().ok_or("An overview needs at least one cell")?;
    Ok(Some(OverviewSelection { extent, primary, cells: resolved.collect() }))
}
