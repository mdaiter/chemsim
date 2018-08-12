#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(non_snake_case)]
#![feature(duration_as_u128)]

extern crate chemsim;
extern crate piston;
extern crate arrayfire;
extern crate conrod;
extern crate gif;
extern crate image;
// extern crate ffmpeg;

use chemsim::display::{Drawable, RGB, PixelPos};
use chemsim::lbm::{Scalar, Matrix};
use arrayfire as af;
use arrayfire::HasAfEnum;

pub fn draw_matrix<T: Copy + HasAfEnum, D: Drawable>(
    buffer: &mut D,
    matrix: &chemsim::matrix::Matrix<T>,
    shader: &(Fn(T) -> i8),
) {
    let (w, h) = matrix.get_shape();
    let copied = matrix.get_underlying();
    for x in 0 .. w {
        for y in 0 .. h {
            let n = shader(copied[(y * w) + x]);
            let k = 2 * (n.abs().min(127) as u8);
            let value = {
                if n < 0 {
                    RGB(k, 0, 0)
                } else if n > 0 {
                    RGB(0, k, 0)
                } else {
                    RGB(0, 0, 0)
                }
            };
            buffer.set_pixel(PixelPos(x as u32, y as u32), value);
        }
    }
}

#[derive(Copy, Clone)]
pub enum DisplayMode {
    Density,
    Speed,
    Velocity,
    MomentumDensity,
}

impl DisplayMode {
    pub fn next(&self) -> DisplayMode {
        match *self {
            DisplayMode::Density         => DisplayMode::Speed,
            DisplayMode::Speed           => DisplayMode::Velocity,
            DisplayMode::Velocity        => DisplayMode::MomentumDensity,
            DisplayMode::MomentumDensity => DisplayMode::Density,
        }
    }
}

pub struct LBMSim {
    speed_factor: usize,
    display_mode: DisplayMode,
    size:         (usize, usize),
    state:        chemsim::lbm::State,
}

impl chemsim::display::Simulation for LBMSim {
    fn size(&self) -> (usize, usize) { self.size }

    fn handle(&mut self, input: &piston::input::Event) {
        use piston::input::*;
        use piston::input::keyboard::*;

        if let Some(Button::Keyboard(k)) = input.release_args() {
            let old_speed_factor = self.speed_factor;

            match k {
                Key::Minus => { self.speed_factor -= 1; },
                Key::Plus  => { self.speed_factor += 1; },
                Key::Space => { self.display_mode = self.display_mode.next(); },
                _          => {},
            };

            if self.speed_factor < 1 {
                self.speed_factor = 1;
            }

            if self.speed_factor > 100 {
                self.speed_factor = 100;
            }

            if old_speed_factor != self.speed_factor {
                println!("Speed factor is now {}", self.speed_factor);
            }
        }

        // FIXME: drawing boundaries etc.
    }

    fn step(&mut self, elapsed: &std::time::Duration) {
        for _ in 0 .. self.speed_factor {
            let t = std::time::Instant::now();
            self.state.step();
            println!("Step {} took {} ms",
                     self.state.time,
                     t.elapsed().as_millis());
        }
    }

    fn render<D: chemsim::display::Drawable>(&self, buf: &mut D) {
        if self.state.is_unstable() {
            println!("[ERROR] Instability detected!");
        }
        println!("Max speed: {}", self.state.speed().maximum_real());
        // for (i, (_, pop)) in self.state.populations().iter().enumerate() {
        //     let fft = pop.dft(1.0).abs();
        //     let nonzeros = af::count_all(fft.get_array()).0 as usize;
        //     let total    = fft.get_width() * fft.get_height();
        //     assert!(total > nonzeros);
        //     let numerator   = total - nonzeros;
        //     let denominator = total;
        //     let ratio       = (100.0 * numerator as f64) / (denominator as f64);
        //     println!("> > FFT of population {} has {} / {} = {}% zeroes",
        //              i, numerator, denominator, ratio);
        // }

        use chemsim::render::*;

        match self.display_mode {
            DisplayMode::Density => {
                render_scalar_field(&self.state.density(), buf);
                println!("Render mode: density");
            },
            DisplayMode::Speed => {
                render_scalar_field(&self.state.speed(), buf);
                println!("Render mode: speed");
            },
            DisplayMode::Velocity => {
                render_vector_field(&self.state.velocity(), buf);
                println!("Render mode: velocity");
            },
            DisplayMode::MomentumDensity => {
                render_vector_field(&self.state.momentum_density(), buf);
                println!("Render mode: momentum density");
            },
        };

        render_geometry(&self.state.geometry, buf);
    }
}

fn initial_state(size: (usize, usize)) -> LBMSim {
    use chemsim::*;

    let (w, h) = size;

    let disc = lbm::Discretization { delta_x: 1.0, delta_t: 1.0 };

    // let collision = lbm::BGK { tau: 15.0 };

    let viscosity = 200.0;
    let collision = lbm::TRT::new(0.25, viscosity, &disc);

    let initial_velocity = {
        let mut vec_x = Vec::new();
        let mut vec_y = Vec::new();
        vec_x.resize(w * h, 0.0);
        vec_y.resize(w * h, 0.0);
        for x in 0 .. w {
            for y in 0 .. h {
                let scale = 0.01;
                // vec_x[(y * w) + x] = -(y as Scalar) * scale / (h as Scalar);
                // vec_y[(y * w) + x] =  (x as Scalar) * scale / (w as Scalar);
                vec_x[(y * w) + x] = scale;
                vec_y[(y * w) + x] = 0.0;
            }
        }
        let vx = matrix::Matrix::new(&vec_x, size).unwrap();
        let vy = matrix::Matrix::new(&vec_y, size).unwrap();
        (vx, vy)
    };

    let initial_density = {
        // FIXME: proper initialization
        // matrix::Matrix::new_filled(0.0, size)
        matrix::Matrix::new_filled(1.0, size)
        // matrix::Matrix::new_random(size).abs().scale(10.0)
        // matrix::Matrix::new_identity(size)

        // let sine = {
        //     let mut vec = Vec::new();
        //     vec.resize(w * h, 0.0);
        //     for x in 0 .. w {
        //         for y in 0 .. h {
        //             let mut val = 0.0;
        //             val += Scalar::sin(3.14159 * (x as Scalar) / (w as Scalar));
        //             val += Scalar::sin(3.14159 * (y as Scalar) / (h as Scalar));
        //             vec[(y * w) + x] = 0.001 * val;
        //         }
        //     }
        //     matrix::Matrix::new(&vec, size).unwrap()
        // };
        // matrix::Matrix::new_filled(1.0, size)
        //     + matrix::Matrix::new_random(size).hadamard(&sine)

        // let mut vec = Vec::new();
        // vec.resize(w * h, 0.0);
        // for x in 0 .. w {
        //     for y in 0 .. h {
        //         let mut val = 0.0;
        //         val += 1.0;
        //         val += 0.3 * Scalar::sin(3.0 * (x as Scalar) / (w as Scalar));
        //         val += 0.3 * Scalar::sin(3.0 * (y as Scalar) / (h as Scalar));
        //         vec[(y * w) + x] = val;
        //     }
        // }
        // matrix::Matrix::new(&vec, size).unwrap()
    };

    let pops = &({
        let temp = lbm::compute_equilibrium(
            initial_density,
            initial_velocity,
            &lbm::D2Q9::directions(),
            disc,
        );
        temp.iter().map(|(_, pop)| pop.clone()).collect::<Vec<lbm::Population>>()
    });

    let lattice = lbm::D2Q9::new(pops);

    let geometry = {
        let mut vec = Vec::new();
        vec.resize(w * h, false);

        {
            let mut set = |x: usize, y: usize, b: bool| { vec[y * w + x] = b; };

            for x in 0 .. w {
                for y in 0 .. h {
                    // set(x, y,
                    //     false
                    //     || (x ==     0) || (y ==     0)
                    //     || (x == w - 1) || (y == h - 1));
                    // set(x, y, (y == 0) || (y == h - 1));
                    let mut r = 0.0;
                    r += (x as f64 - (w as f64 / 2.0)).powi(2);
                    r += (y as f64 - (h as f64 / 2.0)).powi(2);
                    r = r.sqrt();
                    if r < 25.0 {
                        set(x, y, true);
                    }
                }
            }
            // for x in (128 - 51) .. (128 + 51) {
            //     for y in (128 - 50) .. (128 + 50) { set(x, y, true); }
            // }
            // for x in (128 - 49) .. (128 + 49) {
            //     for y in (128 - 49) .. (128 + 49) { set(x, y, false); }
            // }
            //
            // set(128, 128 - 50, false);
            // set(128, 128 - 51, false);
        }

        let vec = vec;

        matrix::Matrix::new(&vec, size).unwrap()
    };

    let state = lbm::State::initial(
        Box::new(lattice),
        geometry,
        Box::new(collision),
        disc,
    );

    LBMSim {
        size:         size,
        state:        state,
        speed_factor: 1,
        display_mode: DisplayMode::Velocity,
    }
}


fn main() -> std::io::Result<()> {
    af::init();
    // ffmpeg::init()?;

    println!("[NOTE] ArrayFire successfully initialized!");

    let recorder = false;
    let (w, h) = (360, 240);

    // -------------------------------------------------------------------------

    use chemsim::display::Simulation;

    let initial = initial_state((w, h));

    chemsim::display::example(initial);

    // if recorder {
    //     let (w, h) = initial.size();
    //
    //     let mut state = initial;
    //     let mut last_draw = std::time::Instant::now();
    //
    //     let mut render_callback = move || -> image::RgbaImage {
    //         let mut rgba_image: image::RgbaImage
    //             = image::ImageBuffer::new(w as u32, h as u32);
    //
    //         // for (_, _, pixel) in rgba_image.enumerate_pixels_mut() {
    //         //     pixel.data = [0, 0, 0, 255];
    //         // }
    //
    //         state.step(&last_draw.elapsed());
    //         last_draw = std::time::Instant::now();
    //         state.render(&mut rgba_image);
    //         rgba_image
    //     };
    //
    //     chemsim::record::record(
    //         (w, h),
    //         std::path::Path::new("output.webm"),
    //         &mut render_callback,
    //         400,
    //         4000,
    //     )?;
    // } else {
    //     chemsim::display::example(initial);
    // }

    Ok(())
}
