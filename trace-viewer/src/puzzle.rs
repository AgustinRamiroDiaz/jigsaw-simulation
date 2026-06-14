use std::collections::HashMap;

use jigsaw_simulation::{
    FirstAgainstRestPickingStrategy, PairPickingSolver, Piece, PuzzleError, RandomPickingStrategy,
    SideIndexedSolver, SolveStep, generate_guid_grid, pieces_from_grid,
};

use crate::image_upload::UploadedImage;
use crate::rng::ViewerRng;
use crate::strategy::SolverStrategy;

const PUZZLE_IMAGE_SIZE: u32 = 1_024;

#[derive(Clone, Debug)]
pub(crate) struct PuzzleImage {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Vec<u8>,
    pub(crate) cols: usize,
    pub(crate) rows: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ImageTile {
    pub(crate) col: usize,
    pub(crate) row: usize,
    pub(crate) clockwise_rotations: u8,
}

pub(crate) struct PuzzleSetup {
    pub(crate) solver: Box<dyn TraceSolver>,
    pub(crate) steps: Vec<SolveStep>,
    pub(crate) image: PuzzleImage,
    pub(crate) image_tiles: HashMap<Piece, ImageTile>,
}

pub(crate) trait TraceSolver: Iterator<Item = Result<SolveStep, PuzzleError>> {
    fn solution(&self) -> Option<Result<Vec<Vec<Piece>>, PuzzleError>>;
}

impl TraceSolver for PairPickingSolver {
    fn solution(&self) -> Option<Result<Vec<Vec<Piece>>, PuzzleError>> {
        self.solution()
    }
}

impl TraceSolver for SideIndexedSolver {
    fn solution(&self) -> Option<Result<Vec<Vec<Piece>>, PuzzleError>> {
        self.solution()
    }
}

pub(crate) fn start_solver(
    width: usize,
    height: usize,
    strategy: SolverStrategy,
    uploaded_image: Option<&UploadedImage>,
) -> Result<PuzzleSetup, String> {
    let mut setup = build_puzzle(width, height, strategy, uploaded_image)
        .map_err(|error| format!("could not start puzzle solver: {error:?}"))?;
    let first_step = setup
        .solver
        .next()
        .ok_or_else(|| String::from("solver did not produce an initial step"))?
        .map_err(|error| format!("could not start puzzle solver: {error:?}"))?;
    setup.steps.push(first_step);

    Ok(setup)
}

fn build_puzzle(
    width: usize,
    height: usize,
    strategy: SolverStrategy,
    uploaded_image: Option<&UploadedImage>,
) -> Result<PuzzleSetup, jigsaw_simulation::PuzzleError> {
    let grid = generate_guid_grid(width, height);
    let image_tiles = image_tile_lookup(&grid);
    let mut pieces = pieces_from_grid(&grid);
    let mut rng = ViewerRng::new((width as u64) << 32 | height as u64);

    pieces.iter_mut().for_each(|piece| {
        *piece = (0..rng.next_index(4)).fold(piece.clone(), |piece, _| piece.rotate_clockwise())
    });

    (1..pieces.len()).rev().for_each(|index| {
        let swap_index = rng.next_index(index + 1);
        pieces.swap(index, swap_index);
    });

    let solver: Box<dyn TraceSolver> = match strategy {
        SolverStrategy::Random => Box::new(PairPickingSolver::with_picking_strategy(
            pieces,
            RandomPickingStrategy::new(9),
        )?),
        SolverStrategy::FirstAgainstRest => Box::new(PairPickingSolver::with_picking_strategy(
            pieces,
            FirstAgainstRestPickingStrategy::new(),
        )?),
        SolverStrategy::SideIndexed => Box::new(SideIndexedSolver::new(pieces)?),
    };

    Ok(PuzzleSetup {
        solver,
        steps: Vec::new(),
        image: PuzzleImage {
            pixels: uploaded_image
                .map(|image| uploaded_puzzle_image(image, width, height))
                .unwrap_or_else(|| generated_puzzle_image(width, height)),
            width: fitted_image_size(width, height).0,
            height: fitted_image_size(width, height).1,
            cols: width,
            rows: height,
        },
        image_tiles,
    })
}

fn image_tile_lookup(grid: &[Vec<Piece>]) -> HashMap<Piece, ImageTile> {
    grid.iter()
        .enumerate()
        .flat_map(|(row, pieces)| {
            pieces.iter().enumerate().flat_map(move |(col, piece)| {
                (0..4).scan(piece.clone(), move |piece, clockwise_rotations| {
                    let rotated = piece.clone();
                    *piece = piece.rotate_clockwise();
                    Some((
                        rotated,
                        ImageTile {
                            col,
                            row,
                            clockwise_rotations,
                        },
                    ))
                })
            })
        })
        .collect()
}

fn generated_puzzle_image(cols: usize, rows: usize) -> Vec<u8> {
    let (width, height) = fitted_image_size(cols, rows);
    (0..height)
        .flat_map(|y| (0..width).flat_map(move |x| landscape_pixel(x, y, width, height)))
        .collect::<Vec<_>>()
}

fn uploaded_puzzle_image(uploaded: &UploadedImage, cols: usize, rows: usize) -> Vec<u8> {
    let source =
        image::RgbaImage::from_raw(uploaded.width, uploaded.height, uploaded.pixels.clone())
            .expect("uploaded image dimensions should match decoded RGBA pixels");
    let (crop_x, crop_y, crop_width, crop_height) =
        center_crop_rect(uploaded.width, uploaded.height, cols, rows);
    let cropped =
        image::imageops::crop_imm(&source, crop_x, crop_y, crop_width, crop_height).to_image();
    let (target_width, target_height) = fitted_image_size(cols, rows);
    let resized = image::imageops::resize(
        &cropped,
        target_width,
        target_height,
        image::imageops::FilterType::Triangle,
    );

    resized.into_raw()
}

fn center_crop_rect(
    source_width: u32,
    source_height: u32,
    cols: usize,
    rows: usize,
) -> (u32, u32, u32, u32) {
    let source_ratio = source_width as f32 / source_height as f32;
    let target_ratio = cols as f32 / rows as f32;

    if source_ratio > target_ratio {
        let crop_width =
            ((source_height as f32 * target_ratio).round() as u32).clamp(1, source_width);
        (
            (source_width - crop_width) / 2,
            0,
            crop_width,
            source_height,
        )
    } else {
        let crop_height =
            ((source_width as f32 / target_ratio).round() as u32).clamp(1, source_height);
        (
            0,
            (source_height - crop_height) / 2,
            source_width,
            crop_height,
        )
    }
}

fn fitted_image_size(cols: usize, rows: usize) -> (u32, u32) {
    if cols >= rows {
        (
            PUZZLE_IMAGE_SIZE,
            ((PUZZLE_IMAGE_SIZE as f32 * rows as f32 / cols as f32).round() as u32).max(1),
        )
    } else {
        (
            ((PUZZLE_IMAGE_SIZE as f32 * cols as f32 / rows as f32).round() as u32).max(1),
            PUZZLE_IMAGE_SIZE,
        )
    }
}

fn landscape_pixel(x: u32, y: u32, width: u32, height: u32) -> [u8; 4] {
    let fx = x as f32 / (width - 1) as f32;
    let fy = y as f32 / (height - 1) as f32;
    let texture = (((x.wrapping_mul(37) ^ y.wrapping_mul(19)) & 0xff) as f32 / 255.0 - 0.5) * 10.0;

    let (mut r, mut g, mut b) = if fy < 0.48 {
        let t = fy / 0.48;
        (42.0 + 72.0 * t, 118.0 + 78.0 * t, 190.0 + 40.0 * t)
    } else if fy < mountain_height(fx, 0.50, 0.10) {
        let shade = (1.0 - fy).clamp(0.0, 1.0);
        (
            62.0 + 50.0 * shade,
            82.0 + 48.0 * shade,
            88.0 + 42.0 * shade,
        )
    } else if fy < mountain_height(fx, 0.62, 0.08) {
        let shade = (1.0 - fy).clamp(0.0, 1.0);
        (
            46.0 + 38.0 * shade,
            92.0 + 62.0 * shade,
            82.0 + 36.0 * shade,
        )
    } else {
        let wave = ((fx * 34.0).sin() + (fy * 48.0).cos()) * 9.0;
        (32.0 + wave, 104.0 + wave, 142.0 + wave * 0.6)
    };

    let sun_dx = fx - 0.76;
    let sun_dy = fy - 0.22;
    let sun = (1.0 - ((sun_dx * sun_dx + sun_dy * sun_dy).sqrt() / 0.18)).clamp(0.0, 1.0);
    r += 130.0 * sun;
    g += 88.0 * sun;
    b += 18.0 * sun;

    let foreground = (fy - 0.72).max(0.0) / 0.28;
    r += foreground * (42.0 + texture);
    g += foreground * (34.0 + texture);
    b += foreground * (18.0 + texture);

    [
        (r + texture).clamp(0.0, 255.0) as u8,
        (g + texture).clamp(0.0, 255.0) as u8,
        (b + texture).clamp(0.0, 255.0) as u8,
        255,
    ]
}

fn mountain_height(x: f32, base: f32, amplitude: f32) -> f32 {
    base + amplitude * (x * std::f32::consts::TAU * 1.6).sin()
        + amplitude * 0.55 * (x * std::f32::consts::TAU * 4.3).cos()
}
