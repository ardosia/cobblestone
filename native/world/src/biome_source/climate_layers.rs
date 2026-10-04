use super::layer::{
    CLIMATE_COLD, CLIMATE_FREEZING, CLIMATE_LUSH, CLIMATE_WARM, DEEP_OCEAN, EdgeMode, Layer,
    LayerRef, MUSHROOM_ISLAND, OCEAN, neighborhood,
};
use super::random::add_coord;

impl Layer {
    pub(super) fn island(&self, x: i32, z: i32, width: usize, height: usize) -> Vec<i32> {
        let mut out = vec![0; width * height];
        for dz in 0..height {
            for dx in 0..width {
                let wx = add_coord(x, dx);
                let wz = add_coord(z, dz);
                out[dx + dz * width] = if self.random.at(wx, wz).next(10) == 0 {
                    CLIMATE_WARM
                } else {
                    OCEAN
                };
            }
        }

        if x <= 0 && z <= 0 && i64::from(x) + width as i64 > 0 && i64::from(z) + height as i64 > 0 {
            let origin_x = usize::try_from(-i64::from(x)).expect("origin x in requested area");
            let origin_z = usize::try_from(-i64::from(z)).expect("origin z in requested area");
            out[origin_x + origin_z * width] = CLIMATE_WARM;
        }

        out
    }

    pub(super) fn add_island(
        &self,
        parent: &LayerRef,
        x: i32,
        z: i32,
        width: usize,
        height: usize,
    ) -> Vec<i32> {
        let stride = width + 2;
        let input = parent.area(x.wrapping_sub(1), z.wrapping_sub(1), stride, height + 2);
        let mut out = vec![0; width * height];

        for dz in 0..height {
            for dx in 0..width {
                let nw = input[dx + dz * stride];
                let ne = input[dx + 2 + dz * stride];
                let sw = input[dx + (dz + 2) * stride];
                let se = input[dx + 2 + (dz + 2) * stride];
                let center = input[dx + 1 + (dz + 1) * stride];
                let mut random = self.random.at(add_coord(x, dx), add_coord(z, dz));

                out[dx + dz * width] = if center == OCEAN
                    && (nw != OCEAN || ne != OCEAN || sw != OCEAN || se != OCEAN)
                {
                    let mut candidate = CLIMATE_WARM;
                    let mut count = 1;
                    for corner in [nw, ne, sw, se] {
                        if corner == OCEAN {
                            continue;
                        }
                        if random.next(count) == 0 {
                            candidate = corner;
                        }
                        count += 1;
                    }

                    if random.next(3) == 0 || candidate == CLIMATE_FREEZING {
                        candidate
                    } else {
                        OCEAN
                    }
                } else if center > OCEAN
                    && (nw == OCEAN || ne == OCEAN || sw == OCEAN || se == OCEAN)
                {
                    if random.next(5) == 0 && center != CLIMATE_FREEZING {
                        OCEAN
                    } else {
                        center
                    }
                } else {
                    center
                };
            }
        }

        out
    }

    pub(super) fn remove_too_much_ocean(
        &self,
        parent: &LayerRef,
        x: i32,
        z: i32,
        width: usize,
        height: usize,
    ) -> Vec<i32> {
        neighborhood(parent, x, z, width, height, |center, n, e, w, s, wx, wz| {
            if center == OCEAN
                && n == OCEAN
                && e == OCEAN
                && w == OCEAN
                && s == OCEAN
                && self.random.at(wx, wz).next(2) == 0
            {
                CLIMATE_WARM
            } else {
                center
            }
        })
    }

    pub(super) fn add_snow(
        &self,
        parent: &LayerRef,
        x: i32,
        z: i32,
        width: usize,
        height: usize,
    ) -> Vec<i32> {
        let input = parent.area(x.wrapping_sub(1), z.wrapping_sub(1), width + 2, height + 2);
        let stride = width + 2;
        let mut out = vec![0; width * height];
        for dz in 0..height {
            for dx in 0..width {
                let center = input[dx + 1 + (dz + 1) * stride];
                out[dx + dz * width] = if center == OCEAN {
                    OCEAN
                } else {
                    match self.random.at(add_coord(x, dx), add_coord(z, dz)).next(6) {
                        0 => CLIMATE_FREEZING,
                        1 => CLIMATE_COLD,
                        _ => CLIMATE_WARM,
                    }
                };
            }
        }
        out
    }

    pub(super) fn add_edge(
        &self,
        parent: &LayerRef,
        mode: EdgeMode,
        x: i32,
        z: i32,
        width: usize,
        height: usize,
    ) -> Vec<i32> {
        if matches!(mode, EdgeMode::Special) {
            return parent
                .area(x, z, width, height)
                .into_iter()
                .enumerate()
                .map(|(index, mut value)| {
                    if value != OCEAN {
                        let dx = index % width;
                        let dz = index / width;
                        let mut random = self.random.at(add_coord(x, dx), add_coord(z, dz));
                        if random.next(13) == 0 {
                            value |= ((random.next(15) + 1) << 8) & 0x0f00;
                        }
                    }
                    value
                })
                .collect();
        }

        neighborhood(
            parent,
            x,
            z,
            width,
            height,
            |center, n, e, w, s, _, _| match mode {
                EdgeMode::CoolWarm if center == CLIMATE_WARM => {
                    if [n, e, w, s]
                        .iter()
                        .any(|value| matches!(*value, CLIMATE_COLD | CLIMATE_FREEZING))
                    {
                        CLIMATE_LUSH
                    } else {
                        center
                    }
                }
                EdgeMode::HeatIce if center == CLIMATE_FREEZING => {
                    if [n, e, w, s]
                        .iter()
                        .any(|value| matches!(*value, CLIMATE_WARM | CLIMATE_LUSH))
                    {
                        CLIMATE_COLD
                    } else {
                        center
                    }
                }
                _ => center,
            },
        )
    }

    pub(super) fn add_mushroom_island(
        &self,
        parent: &LayerRef,
        x: i32,
        z: i32,
        width: usize,
        height: usize,
    ) -> Vec<i32> {
        let stride = width + 2;
        let input = parent.area(x.wrapping_sub(1), z.wrapping_sub(1), stride, height + 2);
        let mut out = vec![0; width * height];
        for dz in 0..height {
            for dx in 0..width {
                let nw = input[dx + dz * stride];
                let ne = input[dx + 2 + dz * stride];
                let sw = input[dx + (dz + 2) * stride];
                let se = input[dx + 2 + (dz + 2) * stride];
                let center = input[dx + 1 + (dz + 1) * stride];
                out[dx + dz * width] = if center == OCEAN
                    && nw == OCEAN
                    && ne == OCEAN
                    && sw == OCEAN
                    && se == OCEAN
                    && self.random.at(add_coord(x, dx), add_coord(z, dz)).next(100) == 0
                {
                    MUSHROOM_ISLAND
                } else {
                    center
                };
            }
        }
        out
    }

    pub(super) fn deep_ocean(
        &self,
        parent: &LayerRef,
        x: i32,
        z: i32,
        width: usize,
        height: usize,
    ) -> Vec<i32> {
        neighborhood(parent, x, z, width, height, |center, n, e, w, s, _, _| {
            if center == OCEAN && [n, e, w, s].iter().all(|value| *value == OCEAN) {
                DEEP_OCEAN
            } else {
                center
            }
        })
    }
}
