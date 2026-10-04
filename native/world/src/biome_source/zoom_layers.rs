use super::layer::{Layer, LayerRef};
use super::random::{ChunkRandom, add_coord};

impl Layer {
    pub(super) fn zoom(
        &self,
        parent: &LayerRef,
        fuzzy: bool,
        x: i32,
        z: i32,
        width: usize,
        height: usize,
    ) -> Vec<i32> {
        let (parent_x, parent_width) = parent_span(x, width, 1);
        let (parent_z, parent_height) = parent_span(z, height, 1);
        let input = parent.area(parent_x, parent_z, parent_width, parent_height);
        let mut out = vec![0; width * height];

        for parent_row in 0..(parent_height - 1) {
            let mut upper_left = input[parent_row * parent_width];
            let mut lower_left = input[(parent_row + 1) * parent_width];

            for parent_column in 0..(parent_width - 1) {
                let upper_right = input[parent_column + 1 + parent_row * parent_width];
                let lower_right = input[parent_column + 1 + (parent_row + 1) * parent_width];
                let world_x = add_coord(parent_x, parent_column).wrapping_mul(2);
                let world_z = add_coord(parent_z, parent_row).wrapping_mul(2);
                let mut random = self.random.at(world_x, world_z);

                // Draw order is observable: the legacy layer chooses the lower-left
                // value first, then the upper-right value, then the diagonal.
                let lower = choose2(&mut random, upper_left, lower_left);
                let right = choose2(&mut random, upper_left, upper_right);
                let diagonal = if fuzzy {
                    choose4(
                        &mut random,
                        upper_left,
                        upper_right,
                        lower_left,
                        lower_right,
                    )
                } else {
                    choose_mode_or_random(
                        &mut random,
                        upper_left,
                        upper_right,
                        lower_left,
                        lower_right,
                    )
                };
                let values = [upper_left, right, lower, diagonal];

                project_cell(
                    &mut out,
                    width,
                    height,
                    x,
                    z,
                    world_x,
                    world_z,
                    2,
                    |local_x, local_z| values[local_x + local_z * 2],
                );

                upper_left = upper_right;
                lower_left = lower_right;
            }
        }

        out
    }

    pub(super) fn voronoi(
        &self,
        parent: &LayerRef,
        x: i32,
        z: i32,
        width: usize,
        height: usize,
    ) -> Vec<i32> {
        let shifted_x = x.wrapping_sub(2);
        let shifted_z = z.wrapping_sub(2);
        let (parent_x, parent_width) = parent_span(shifted_x, width, 2);
        let (parent_z, parent_height) = parent_span(shifted_z, height, 2);
        let input = parent.area(parent_x, parent_z, parent_width, parent_height);
        let mut shifted_out = vec![0; width * height];

        for parent_row in 0..(parent_height - 1) {
            let mut upper_left = input[parent_row * parent_width] & 0xff;
            let mut lower_left = input[(parent_row + 1) * parent_width] & 0xff;

            for parent_column in 0..(parent_width - 1) {
                let upper_right = input[parent_column + 1 + parent_row * parent_width] & 0xff;
                let lower_right = input[parent_column + 1 + (parent_row + 1) * parent_width] & 0xff;

                let cell_x = add_coord(parent_x, parent_column).wrapping_mul(4);
                let cell_z = add_coord(parent_z, parent_row).wrapping_mul(4);
                let (upper_left_x, upper_left_z) = jitter(&self.random, cell_x, cell_z, 0.0, 0.0);
                let (upper_right_x, upper_right_z) =
                    jitter(&self.random, cell_x.wrapping_add(4), cell_z, 4.0, 0.0);
                let (lower_left_x, lower_left_z) =
                    jitter(&self.random, cell_x, cell_z.wrapping_add(4), 0.0, 4.0);
                let (lower_right_x, lower_right_z) = jitter(
                    &self.random,
                    cell_x.wrapping_add(4),
                    cell_z.wrapping_add(4),
                    4.0,
                    4.0,
                );

                project_cell(
                    &mut shifted_out,
                    width,
                    height,
                    shifted_x,
                    shifted_z,
                    cell_x,
                    cell_z,
                    4,
                    |local_x, local_z| {
                        let x_here = local_x as f64;
                        let z_here = local_z as f64;
                        let d0 = distance_squared(x_here, z_here, upper_left_x, upper_left_z);
                        let d1 = distance_squared(x_here, z_here, upper_right_x, upper_right_z);
                        let d2 = distance_squared(x_here, z_here, lower_left_x, lower_left_z);
                        let d3 = distance_squared(x_here, z_here, lower_right_x, lower_right_z);

                        if d0 < d1 && d0 < d2 && d0 < d3 {
                            upper_left
                        } else if d1 < d0 && d1 < d2 && d1 < d3 {
                            upper_right
                        } else if d2 < d0 && d2 < d1 && d2 < d3 {
                            lower_left
                        } else {
                            lower_right
                        }
                    },
                );

                upper_left = upper_right;
                lower_left = lower_right;
            }
        }

        shifted_out
    }
}

fn parent_span(origin: i32, length: usize, shift: u32) -> (i32, usize) {
    debug_assert!(length > 0);
    let start = i64::from(origin) >> shift;
    let end_coordinate =
        i64::from(origin) + i64::try_from(length - 1).expect("area length fits i64");
    let end = end_coordinate >> shift;
    let span = usize::try_from(end - start + 2).expect("parent span is positive");
    (
        i32::try_from(start).expect("scaled parent coordinate fits i32"),
        span,
    )
}

#[allow(clippy::too_many_arguments)]
fn project_cell(
    out: &mut [i32],
    width: usize,
    height: usize,
    area_x: i32,
    area_z: i32,
    cell_x: i32,
    cell_z: i32,
    edge: usize,
    mut value: impl FnMut(usize, usize) -> i32,
) {
    let relative_x = i64::from(cell_x) - i64::from(area_x);
    let relative_z = i64::from(cell_z) - i64::from(area_z);

    for local_z in 0..edge {
        let out_z = relative_z + local_z as i64;
        if out_z < 0 || out_z >= height as i64 {
            continue;
        }
        for local_x in 0..edge {
            let out_x = relative_x + local_x as i64;
            if out_x < 0 || out_x >= width as i64 {
                continue;
            }
            out[out_x as usize + out_z as usize * width] = value(local_x, local_z);
        }
    }
}

fn jitter(
    random: &super::random::LayerRandom,
    x: i32,
    z: i32,
    x_offset: f64,
    z_offset: f64,
) -> (f64, f64) {
    let mut random = random.at(x, z);
    let jitter_x = (f64::from(random.next(1024)) / 1024.0 - 0.5) * 3.6 + x_offset;
    let jitter_z = (f64::from(random.next(1024)) / 1024.0 - 0.5) * 3.6 + z_offset;
    (jitter_x, jitter_z)
}

fn distance_squared(x: f64, z: f64, target_x: f64, target_z: f64) -> f64 {
    let dx = x - target_x;
    let dz = z - target_z;
    dx * dx + dz * dz
}

fn choose2(random: &mut ChunkRandom, first: i32, second: i32) -> i32 {
    if random.next(2) == 0 { first } else { second }
}

fn choose4(random: &mut ChunkRandom, a: i32, b: i32, c: i32, d: i32) -> i32 {
    match random.next(4) {
        0 => a,
        1 => b,
        2 => c,
        _ => d,
    }
}

fn choose_mode_or_random(random: &mut ChunkRandom, a: i32, b: i32, c: i32, d: i32) -> i32 {
    if b == c && c == d {
        return b;
    }
    if a == b && a == c {
        return a;
    }
    if a == b && a == d {
        return a;
    }
    if a == c && a == d {
        return a;
    }
    if a == b && c != d {
        return a;
    }
    if a == c && b != d {
        return a;
    }
    if a == d && b != c {
        return a;
    }
    if b == c && a != d {
        return b;
    }
    if b == d && a != c {
        return b;
    }
    if c == d && a != b {
        return c;
    }
    choose4(random, a, b, c, d)
}
