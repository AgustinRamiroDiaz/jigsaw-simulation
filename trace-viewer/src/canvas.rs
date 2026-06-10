use std::collections::HashMap;
use std::fmt::Debug;

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, TextureHandle, Ui, Vec2};
use jigsaw_simulation::{Direction, Piece, SolveStep, TracePolyomino};

use crate::puzzle::{ImageTile, PuzzleImage};

pub(crate) fn draw_trace_canvas(
    ui: &mut Ui,
    step: &SolveStep,
    image: &PuzzleImage,
    image_tiles: &HashMap<Piece, ImageTile>,
    texture: Option<&TextureHandle>,
) {
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 0.0, Color32::from_rgb(245, 247, 250));

    let layout = layout_polyominos(&step.polyominos, rect.width());

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

        draw_side_colors(painter, &cell.piece, rect);
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

fn draw_side_colors(painter: &egui::Painter, piece: &Piece, rect: Rect) {
    let size = rect.width();
    let thickness = (size * 0.14).clamp(1.5, 5.0);

    [
        (
            Direction::Top,
            Rect::from_min_size(rect.min, Vec2::new(size, thickness)),
        ),
        (
            Direction::Right,
            Rect::from_min_size(
                Pos2::new(rect.right() - thickness, rect.top()),
                Vec2::new(thickness, size),
            ),
        ),
        (
            Direction::Bottom,
            Rect::from_min_size(
                Pos2::new(rect.left(), rect.bottom() - thickness),
                Vec2::new(size, thickness),
            ),
        ),
        (
            Direction::Left,
            Rect::from_min_size(rect.min, Vec2::new(thickness, size)),
        ),
    ]
    .into_iter()
    .for_each(|(direction, rect)| {
        let color = color_for_side(piece.side(direction));
        painter.rect_filled(rect, 0.0, color.linear_multiply(0.58));
    });
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

fn color_for_side(side: &impl Debug) -> Color32 {
    let hash = format!("{side:?}").bytes().fold(0_u32, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as u32)
    });

    hsl_to_rgb((hash % 360) as f32, 0.68, 0.56)
}

fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> Color32 {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let hue_prime = hue / 60.0;
    let x = chroma * (1.0 - (hue_prime % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match hue_prime as u8 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = lightness - chroma / 2.0;

    Color32::from_rgb(
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
    )
}
