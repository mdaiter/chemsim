#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(non_snake_case)]
#![feature(duration_as_u128)]

extern crate chemsim;
extern crate piston;
extern crate arrayfire;
extern crate conrod;

use chemsim::display::Drawable;
use chemsim::lbm::{Scalar, Matrix};
use arrayfire::HasAfEnum;

pub fn draw_matrix<T: Copy + HasAfEnum, D: Drawable>(
    buffer: &mut D,
    matrix: &chemsim::matrix::Matrix<T>,
    shader: &(Fn(T) -> u8),
) {
    use chemsim::lbm::Scalar;
    use chemsim::display::{RGB, PixelPos};
    let (w, h) = matrix.get_shape();
    let copied = matrix.get_underlying();
    for x in 0 .. w {
        for y in 0 .. h {
            let n = shader(copied[(y * w) + x]);
            let value = RGB(n, n, n);
            buffer.set_pixel(PixelPos(x as u32, y as u32), value);
        }
    }
}

pub struct LBMSim {
    size:  (usize, usize),
    state: chemsim::lbm::State,
}

impl LBMSim {
    pub fn draw(&self) -> chemsim::lbm::Matrix {
        if self.state.is_unstable() {
            println!("[ERROR] Instability detected!");
        }
        self.state.density()
    }
}

impl chemsim::display::Simulation for LBMSim {
    fn size(&self) -> (usize, usize) {
        self.size
    }

    fn handle(&mut self, input: &piston::input::Event) {
        // FIXME: drawing boundaries etc.
    }

    fn step(&mut self, elapsed: &std::time::Duration) {
        let t = std::time::Instant::now();
        self.state.step();
        println!("Step {} took {} ms",
                 self.state.time,
                 t.elapsed().as_millis());
    }

    fn render<D: chemsim::display::Drawable>(&self, buf: &mut D) {
        let matrix = self.draw();
        assert_eq!(matrix.get_shape(), self.size);
        let max = f64::max(
            arrayfire::imax_all(matrix.abs().get_array()).0 as Scalar,
            0.1,
        );
        draw_matrix(buf, &matrix, &(|value: Scalar| -> u8 {
            let f = 256.0 * Scalar::abs(value / max);
            f.min(255.0).max(0.0) as u8
        }));
    }
}

fn initial_state(size: (usize, usize)) -> LBMSim {
    use chemsim::*;

    let collision = lbm::BGK { tau: 1.0 };

    let initial_density = {
        // FIXME: proper initialization
        // matrix::Matrix::new_filled(0.0, size)
        // matrix::Matrix::new_filled(1.0, size)
        // matrix::Matrix::new_random(size).abs().scale(0.0001)
        // matrix::Matrix::new_identity(size)
        let (w, h) = size;
        let mut vec = Vec::new();
        vec.resize(w * h, 0.0);
        for x in 0 .. w {
            for y in 0 .. h {
                let mut val = 0.0;
                val += Scalar::powi((x as Scalar) / (w as Scalar), 2);
                val += Scalar::powi((y as Scalar) / (h as Scalar), 2);
                vec[(y * w) + x] = val; // (1000000.0 * val * val) * std::f32::MIN_POSITIVE;
            }
        }
        matrix::Matrix::new(&vec, size).unwrap()
    };

    let pops = &[
        initial_density.scale(16.0 / 36.0),
        initial_density.scale(4.0  / 36.0),
        initial_density.scale(4.0  / 36.0),
        initial_density.scale(4.0  / 36.0),
        initial_density.scale(4.0  / 36.0),
        initial_density.scale(1.0  / 36.0),
        initial_density.scale(1.0  / 36.0),
        initial_density.scale(1.0  / 36.0),
        initial_density.scale(1.0  / 36.0),
    ];

    let lattice = lbm::Lattice::new_D2Q9(pops);

    let disc = lbm::Discretization { delta_x: 1.0, delta_t: 1.0 };

    let state = lbm::State::initial(lattice, disc, Box::new(collision));

    LBMSim { size: size, state: state }
}

extern crate gif;
use std::fs::File;

fn main() {
    // chemsim::display::conrod();
    arrayfire::init();
    chemsim::display::example(initial_state((500, 500)));

    // use gif::SetParameter;
    // let (w, h) = (500, 500);
    // let initial = initial_state((w, h));
    // let color_map = &[0xFF, 0xFF, 0xFF, 0, 0, 0];
    // let mut image = File::create("output.gif").unwrap();
    // let mut encoder
    //     = gif::Encoder::new(&mut image, w as u16, h as u16, color_map).unwrap();
    // encoder.set(gif::Repeat::Infinite).unwrap();
    // chemsim::display::record(initial, 350, &mut encoder).unwrap();
}
