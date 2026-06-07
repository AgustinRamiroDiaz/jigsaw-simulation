use std::collections::HashMap;
use std::fmt::Debug;

use iced::mouse;
use iced::widget::canvas;
use iced::{Color, Point as CanvasPoint, Rectangle, Renderer, Size, Theme, Vector};
use jigsaw_simulation::{Direction, Piece, SolveStep, TracePolyomino};

use crate::puzzle::{ImageTile, PuzzleImage};

#[derive(Clone, Debug)]
pub(crate) struct TraceCanvas {
    pub(crate) step: SolveStep,
    pub(crate) image: PuzzleImage,
    pub(crate) image_tiles: HashMap<Piece, ImageTile>,
}

impl<Message> canvas::Program<Message> for TraceCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let background = canvas::Path::rectangle(CanvasPoint::ORIGIN, bounds.size());
        frame.fill(&background, Color::from_rgb8(245, 247, 250));

        let layout = layout_polyominos(&self.step.polyominos, bounds.width);

        layout.iter().for_each(|entry| {
            draw_polyomino(
                &mut frame,
                entry.polyomino,
                entry.origin,
                entry.cell_size,
                &self.image,
                &self.image_tiles,
            );
        });

        vec![frame.into_geometry()]
    }
}

#[derive(Clone, Debug)]
struct PolyominoLayout<'a> {
    polyomino: &'a TracePolyomino,
    origin: CanvasPoint,
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

                let origin = CanvasPoint::new(*x, *y);
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
    frame: &mut canvas::Frame,
    polyomino: &TracePolyomino,
    origin: CanvasPoint,
    cell_size: f32,
    image: &PuzzleImage,
    image_tiles: &HashMap<Piece, ImageTile>,
) {
    polyomino.cells.iter().for_each(|cell| {
        let x = origin.x + cell.point.x as f32 * cell_size;
        let y = origin.y + cell.point.y as f32 * cell_size;
        let size = cell_size - 2.0;
        let rect = canvas::Path::rectangle(CanvasPoint::new(x, y), Size::new(size, size));

        if let Some(tile) = image_tiles.get(&cell.piece) {
            draw_image_tile(frame, image, *tile, CanvasPoint::new(x, y), size);
        } else {
            frame.fill(&rect, Color::from_rgb8(202, 206, 211));
        }

        draw_side_colors(frame, &cell.piece, CanvasPoint::new(x, y), size);

        frame.stroke(
            &rect,
            canvas::Stroke::default()
                .with_color(Color::from_rgb8(42, 48, 58))
                .with_width(1.0),
        );
    });
}

fn draw_image_tile(
    frame: &mut canvas::Frame,
    image: &PuzzleImage,
    tile: ImageTile,
    origin: CanvasPoint,
    size: f32,
) {
    let clip = Rectangle::new(origin, Size::new(size, size));
    let full_width = image.cols as f32 * size;
    let full_height = image.rows as f32 * size;
    let full_origin = CanvasPoint::new(
        origin.x - tile.col as f32 * size,
        origin.y - tile.row as f32 * size,
    );

    frame.with_clip(clip, |frame| {
        frame.with_save(|frame| {
            let center = Vector::new(origin.x + size / 2.0, origin.y + size / 2.0);
            frame.translate(center);
            frame.rotate(std::f32::consts::FRAC_PI_2 * tile.clockwise_rotations as f32);
            frame.translate(Vector::new(-center.x, -center.y));
            frame.draw_image(
                Rectangle::new(full_origin, Size::new(full_width, full_height)),
                &image.handle,
            );
        });
    });
}

fn draw_side_colors(frame: &mut canvas::Frame, piece: &Piece, origin: CanvasPoint, size: f32) {
    let thickness = (size * 0.14).clamp(1.5, 5.0);

    [
        (Direction::Top, origin, Size::new(size, thickness)),
        (
            Direction::Right,
            CanvasPoint::new(origin.x + size - thickness, origin.y),
            Size::new(thickness, size),
        ),
        (
            Direction::Bottom,
            CanvasPoint::new(origin.x, origin.y + size - thickness),
            Size::new(size, thickness),
        ),
        (Direction::Left, origin, Size::new(thickness, size)),
    ]
    .into_iter()
    .for_each(|(direction, origin, size)| {
        let edge = canvas::Path::rectangle(origin, size);
        let color = color_for_side(piece.side(direction));
        frame.fill(&edge, Color::from_rgba(color.r, color.g, color.b, 0.58));
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

fn color_for_side(side: &impl Debug) -> Color {
    let hash = format!("{side:?}").bytes().fold(0_u32, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as u32)
    });

    hsl_to_rgb((hash % 360) as f32, 0.68, 0.56)
}

fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> Color {
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

    Color::from_rgb(r1 + m, g1 + m, b1 + m)
}
