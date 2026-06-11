use std::collections::HashMap;

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, TextureHandle, Ui, Vec2};
use jigsaw_simulation::{Piece, SolveStep, TracePolyomino};

use crate::puzzle::{ImageTile, PuzzleImage};

pub(crate) fn draw_trace_canvas(
    ui: &mut Ui,
    step: &SolveStep,
    image: &PuzzleImage,
    image_tiles: &HashMap<Piece, ImageTile>,
    texture: Option<&TextureHandle>,
) {
    let width = ui.available_width().max(1.0);
    let layout = layout_polyominos(&step.polyominos, width);
    let height = layout_height(&layout).max(360.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 0.0, Color32::from_rgb(245, 247, 250));

    layout.iter().for_each(|entry| {
        draw_polyomino(
            &painter,
            entry.polyomino,
            rect.min + entry.origin.to_vec2(),
            entry.cell_size,
            image,
            image_tiles,
            texture,
        );
    });
}

#[derive(Clone, Debug)]
struct PolyominoLayout<'a> {
    polyomino: &'a TracePolyomino,
    origin: Pos2,
    cell_size: f32,
}

fn layout_polyominos(polyominos: &[TracePolyomino], width: f32) -> Vec<PolyominoLayout<'_>> {
    let margin = 24.0;
    let gap = 20.0;
    let cell_size = if polyominos.len() > 30 { 12.0 } else { 24.0 };

    polyominos
        .iter()
        .scan(
            (margin, margin, 0.0_f32),
            |(x, y, row_height), polyomino| {
                let (poly_width, poly_height) = polyomino_size(polyomino, cell_size);

                if *x + poly_width > width - margin && *x > margin {
                    *x = margin;
                    *y += *row_height + gap;
                    *row_height = 0.0;
                }

                let origin = Pos2::new(*x, *y);
                *x += poly_width + gap;
                *row_height = row_height.max(poly_height);

                Some(PolyominoLayout {
                    polyomino,
                    origin,
                    cell_size,
                })
            },
        )
        .collect()
}

fn layout_height(layout: &[PolyominoLayout<'_>]) -> f32 {
    let margin = 24.0;

    layout
        .iter()
        .map(|entry| entry.origin.y + polyomino_size(entry.polyomino, entry.cell_size).1)
        .fold(margin, f32::max)
        + margin
}

fn draw_polyomino(
    painter: &egui::Painter,
    polyomino: &TracePolyomino,
    origin: Pos2,
    cell_size: f32,
    image: &PuzzleImage,
    image_tiles: &HashMap<Piece, ImageTile>,
    texture: Option<&TextureHandle>,
) {
    polyomino.cells.iter().for_each(|cell| {
        let x = origin.x + cell.point.x as f32 * cell_size;
        let y = origin.y + cell.point.y as f32 * cell_size;
        let size = cell_size - 2.0;
        let rect = Rect::from_min_size(Pos2::new(x, y), Vec2::splat(size));

        if let (Some(tile), Some(texture)) = (image_tiles.get(&cell.piece), texture) {
            draw_image_tile(painter, image, *tile, rect, texture);
        } else {
            painter.rect_filled(rect, 0.0, Color32::from_rgb(202, 206, 211));
        }

        painter.rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, Color32::from_rgb(42, 48, 58)),
            egui::StrokeKind::Inside,
        );
    });
}

fn draw_image_tile(
    painter: &egui::Painter,
    image: &PuzzleImage,
    tile: ImageTile,
    rect: Rect,
    texture: &TextureHandle,
) {
    let col_width = 1.0 / image.cols as f32;
    let row_height = 1.0 / image.rows as f32;
    let uv = Rect::from_min_max(
        Pos2::new(tile.col as f32 * col_width, tile.row as f32 * row_height),
        Pos2::new(
            (tile.col + 1) as f32 * col_width,
            (tile.row + 1) as f32 * row_height,
        ),
    );

    let positions = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    let mut uvs = [
        uv.left_top(),
        uv.right_top(),
        uv.right_bottom(),
        uv.left_bottom(),
    ];
    uvs.rotate_right(tile.clockwise_rotations as usize % 4);

    let mut mesh = egui::Mesh::with_texture(texture.id());
    let index_start = mesh.vertices.len() as u32;

    positions.into_iter().zip(uvs).for_each(|(pos, uv)| {
        mesh.vertices.push(egui::epaint::Vertex {
            pos,
            uv,
            color: Color32::WHITE,
        });
    });

    mesh.indices.extend_from_slice(&[
        index_start,
        index_start + 1,
        index_start + 2,
        index_start,
        index_start + 2,
        index_start + 3,
    ]);

    painter.add(egui::Shape::mesh(mesh));
}

fn polyomino_size(polyomino: &TracePolyomino, cell_size: f32) -> (f32, f32) {
    let max_x = polyomino
        .cells
        .iter()
        .map(|cell| cell.point.x)
        .max()
        .unwrap_or(0);
    let max_y = polyomino
        .cells
        .iter()
        .map(|cell| cell.point.y)
        .max()
        .unwrap_or(0);

    (
        (max_x + 1) as f32 * cell_size,
        (max_y + 1) as f32 * cell_size,
    )
}
