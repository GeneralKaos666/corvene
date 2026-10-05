//! History's commit graph column (Corvene `1213-commit-graph`; GHD
//! `ui/history/commit-list-item.tsx` has none). The lanes are laid out once
//! in `corvene_core::commit_graph`; each virtualised row paints only its own
//! segment: lines from the top of the row, the commit's dot, lines to the
//! bottom. Past [`MAX_COLUMNS`] the last column stands for the hidden lanes
//! (a dotted line), and lines into them end there.

use std::cell::RefCell;
use std::rc::Rc;

use corvene_core::commit_graph::{CommitGraph, EdgeKind, PALETTE_LEN};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::theme::ActiveGhdTheme;
use crate::theme::c;
use crate::theme::primer::*;
use crate::theme::sizes::*;

/// The most columns the graph takes; with more lanes the last of them is
/// the overflow column.
pub const MAX_COLUMNS: usize = 8;

fn lane_width() -> Pixels {
    zpx(12.)
}

/// Before the first lane; the contents' own 10 px padding follows the graph.
fn left_pad() -> Pixels {
    zpx(4.)
}

/// The columns a graph `width` lanes wide shows.
pub fn graph_columns(width: usize) -> usize {
    width.clamp(1, MAX_COLUMNS)
}

/// The graph column's width for `columns`.
pub fn column_width(columns: usize) -> Pixels {
    left_pad() + lane_width() * columns as f32
}

/// The lane colours: 0 (the current branch) is the accent blue.
pub fn palette(cx: &App) -> [Hsla; PALETTE_LEN as usize] {
    if cx.ghd().is_dark() {
        [
            c(BLUE_400),
            c(GREEN_400),
            c(ORANGE_400),
            c(PURPLE_300),
            c(RED_400),
            c(YELLOW_500),
            c(GRAY_400),
        ]
    } else {
        [
            c(BLUE_500),
            c(GREEN_500),
            c(ORANGE_500),
            c(PURPLE_500),
            c(RED_500),
            c(YELLOW_700),
            c(GRAY_500),
        ]
    }
}

/// How a row paints its segment.
#[derive(Clone, Copy)]
pub struct GraphStyle {
    pub palette: [Hsla; PALETTE_LEN as usize],
    /// Replaces colour 0 where the row's background is the accent (the
    /// focused selection), which would hide it.
    pub accent: Option<Hsla>,
    /// The row's background: a merge's dot is a ring around it.
    pub background: Hsla,
    /// The overflow column's dots.
    pub overflow: Hsla,
    /// The row's height, its bottom border included (lanes run through it).
    pub height: Pixels,
}

/// Row `ix`'s graph cell, `columns` wide.
pub fn graph_cell(
    graph: Rc<RefCell<CommitGraph>>,
    ix: usize,
    columns: usize,
    style: GraphStyle,
) -> AnyElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            paint_row(&graph.borrow(), ix, bounds.origin, &style, window);
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .w(column_width(columns))
    .h(style.height)
    .into_any_element()
}

fn paint_row(
    graph: &CommitGraph,
    ix: usize,
    origin: Point<Pixels>,
    style: &GraphStyle,
    window: &mut Window,
) {
    let Some((row, edges)) = graph.row(ix) else {
        return;
    };
    let overflow_col = (graph.width() > MAX_COLUMNS).then_some(MAX_COLUMNS - 1);
    let hidden = |c: u16| overflow_col.is_some_and(|o| c as usize >= o);
    let col = |c: u16| overflow_col.map_or(c as usize, |o| (c as usize).min(o));
    let x = |c: usize| origin.x + left_pad() + lane_width() * (c as f32 + 0.5);
    let top = origin.y;
    let bottom = origin.y + style.height;
    let mid = origin.y + style.height / 2.;
    let color = |i: u8| {
        let c = style.palette[(i % PALETTE_LEN) as usize];
        if i == 0 { style.accent.unwrap_or(c) } else { c }
    };

    // straight lines are quads (most of a graph, and much cheaper than
    // tessellated paths); curves go in one path per colour and weight; the
    // emphasised lane on top
    let width = |emph: bool| if emph { zpx(2.5) } else { zpx(1.5) };
    let mut lines: Vec<(bool, PaintQuad)> = Vec::new();
    let mut curves: Vec<(u8, bool, PathBuilder)> = Vec::new();
    let mut overflow = false;
    for e in edges {
        if hidden(e.from) || hidden(e.to) {
            overflow = true;
            if hidden(e.from) && hidden(e.to) {
                continue;
            }
        }
        let (from, to) = (col(e.from), col(e.to));
        let (a, b) = match e.kind {
            EdgeKind::Through => (point(x(from), top), point(x(to), bottom)),
            EdgeKind::In => (point(x(from), top), point(x(to), mid)),
            EdgeKind::Out => (point(x(from), mid), point(x(to), bottom)),
        };
        let w = width(e.emphasized);
        if from == to {
            let bounds = Bounds::new(point(a.x - w / 2., a.y), size(w, b.y - a.y));
            lines.push((e.emphasized, fill(bounds, color(e.color))));
            continue;
        }
        let ix = match curves
            .iter()
            .position(|(c, emph, _)| *c == e.color && *emph == e.emphasized)
        {
            Some(ix) => ix,
            None => {
                curves.push((e.color, e.emphasized, PathBuilder::stroke(w)));
                curves.len() - 1
            }
        };
        // leaves and arrives vertically
        let half = (b.y - a.y) / 2.;
        let path = &mut curves[ix].2;
        path.move_to(a);
        path.cubic_bezier_to(b, point(a.x, a.y + half), point(b.x, a.y + half));
    }
    for pass in [false, true] {
        for (_, quad) in lines.iter().filter(|(emph, _)| *emph == pass) {
            window.paint_quad(quad.clone());
        }
        for (c, _, path) in curves.iter_mut().filter(|(_, emph, _)| *emph == pass) {
            let path = std::mem::replace(path, PathBuilder::stroke(px(0.)));
            if let Ok(path) = path.build() {
                window.paint_path(path, color(*c));
            }
        }
    }

    if let Some(o) = overflow_col.filter(|_| overflow) {
        let dot = zpx(1.5);
        let mut y = top + zpx(1.);
        while y + dot <= bottom {
            window.paint_quad(fill(
                Bounds::new(point(x(o) - dot / 2., y), size(dot, dot)),
                style.overflow,
            ));
            y += zpx(4.);
        }
    }

    let radius = if row.emphasized { zpx(4.5) } else { zpx(4.) };
    let center = point(x(col(row.lane)), mid);
    let bounds = Bounds::new(
        point(center.x - radius, center.y - radius),
        size(radius * 2., radius * 2.),
    );
    let fg = color(row.color);
    if row.merge {
        window.paint_quad(quad(
            bounds,
            radius,
            style.background,
            zpx(2.),
            fg,
            BorderStyle::Solid,
        ));
    } else {
        window.paint_quad(fill(bounds, fg).corner_radii(radius));
    }
}
