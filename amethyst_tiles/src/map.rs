#![allow(unused_variables)]

use crate::CoordinateEncoder;
use amethyst_assets::{Asset, Handle};
use amethyst_core::{
    ecs::{Component, HashMapStorage, World},
    math::{Matrix4, Point3, Vector3},
};
use amethyst_rendy::{palette::Srgba, SpriteSheet};

/// Trait providing generic rendering functionality to all tiles. Using a tilemap requires you to provide a `Tile` type,
/// which must implement this trait to provide the `RenderPass` with the appropriate sprite and tint values.
pub trait Tile: 'static + Clone + Send + Sync + Default {
    /// Takes an immutable reference to world to process this sprite and return its sprite.
    fn sprite(&self, coordinates: Point3<u32>, world: &World) -> Option<usize> {
        None
    }

    /// Takes an immutable reference to world to process this sprite and return its tint.
    fn tint(&self, coordinates: Point3<u32>, world: &World) -> Srgba {
        Srgba::new(1.0, 1.0, 1.0, 1.0)
    }
}

/// Trait for providing access to an underlying storage type of a 3-dimensional Tile data. This is abstracted to provide
/// for allowing more underlying storage types in the future beyond a flat array, such as networking, chunking, etc.
pub trait Map {
    /// The world-space (Amethyst) dimensions of a single tile in this map space (1x1x1). This is used to scale our
    /// sprites to the world coordinate space. This should usually be the tile sprite dimensions. Beware, Z-size is taken
    /// into consideration as well so you will usually want to se Z-size to 1.
    fn tile_dimensions(&self) -> &Vector3<u32>;

    /// The dimensions of this map instance.
    fn dimensions(&self) -> &Vector3<u32>;

    /// The origin coordinate of this map instance. Usually 0,0,0. This is allowed for offseting the map off an origin.
    fn origin(&self) -> &Point3<f32>;

    /// Set the sprite sheet handle which the tile render pass should use for rendering this map.
    fn set_sprite_sheet(&mut self, sprite_sheet: Option<Handle<SpriteSheet>>);

    /// Convert a tile coordinate `Point3<u32>` to an amethyst world-coordinate space coordinate `Point3<f32>`
    /// This performs an inverse matrix transformation of the world coordinate, scaling and translating using this
    /// maps `origin` and `tile_dimensions` respectively.
    fn to_world(&self, coord: &Point3<u32>) -> Vector3<f32>;

    /// Convert an amethyst world-coordinate space coordinate `Point3<f32>` to a tile coordinate `Point3<u32>`
    /// This performs an inverse matrix transformation of the world coordinate, scaling and translating using this
    /// maps `origin` and `tile_dimensions` respectively.
    fn to_tile(&self, coord: &Vector3<f32>) -> Option<Point3<u32>>;

    /// Returns the `Matrix4` transform which was created for transforming between world and tile coordinate spaces.
    fn transform(&self) -> &Matrix4<f32>;

    /// Call the underlying coordinate encoder for this map instance, which should always reduce to a u32 integer.
    fn encode(&self, coord: &Point3<u32>) -> Option<u32>;

    /// Call the underlying coordinate encoder for this map instance, which should always reduce to a u32 integer.
    fn encode_raw(&self, coord: &(u32, u32, u32)) -> Option<u32>;

    /// Call the underlying coordinate decoder for this map instance, which should always convert a u32 to a tile-space
    /// coordinate.
    fn decode(&self, morton: u32) -> Option<Point3<u32>>;

    /// Call the underlying coordinate decoder for this map instance, which should always convert a u32 to a tile-space
    /// coordinate.
    fn decode_raw(&self, morton: u32) -> Option<(u32, u32, u32)>;
}

/// Generic trait over the underlying storage of a given `Map` type.
pub trait MapStorage<T: Tile> {
    /// Try to get the `Tile` type present at the provided coordinates.
    fn get(&self, coord: &Point3<u32>) -> Option<&T>;

    /// Try to get a mutable reference to `Tile` type present at the provided coordinates.
    fn get_mut(&mut self, coord: &Point3<u32>) -> Option<&mut T>;

    /// Try to get a mutable reference to `Tile` type present at the provided coordinates.
    fn get_mut_nochange(&mut self, coord: &Point3<u32>) -> Option<&mut T>;

    /// Try to get the `Tile` type present at the provided coordinates.
    fn get_raw(&self, coord: u32) -> Option<&T>;

    /// Try to get a mutable reference to `Tile` type present at the provided coordinates.
    fn get_raw_mut(&mut self, coord: u32) -> Option<&mut T>;

    /// Try to get a mutable reference to `Tile` type present at the provided coordinates.
    fn get_raw_mut_nochange(&mut self, coord: u32) -> Option<&mut T>;
}

/// Concrete implementation of a generic 3D `TileMap` component. Accepts a `Tile` type and `CoordinateEncoder` type,
/// creating a flat 1D array storage which is spatially partitioned utilizing the provided encoding scheme.
/// creating a flat 1D array storage which is spatially partitioned utilizing the provided encoding scheme.
///
/// The defualt encoding scheme is `MortonEncoder2D`, which allows for arbitrary X, Y and Z coordinate sizes while
/// still spatially partitioning each z-level. For more efficient Z-order encoding, use `MortonEncoder` which requires
/// cubic map dimensions but provides for much greater spatial efficiency.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TileMap<T: Tile, E: CoordinateEncoder = crate::MortonEncoder2D> {
    pub(crate) origin: Point3<f32>,
    pub(crate) tile_dimensions: Vector3<u32>,
    pub(crate) dimensions: Vector3<u32>,
    pub(crate) transform: Matrix4<f32>,

    pub(crate) version: u64,

    #[serde(skip)]
    pub(crate) sprite_sheet: Option<Handle<SpriteSheet>>,

    pub(crate) data: Vec<T>,

    #[serde(skip)]
    pub(crate) encoder: E,
}
impl<T: Tile, E: CoordinateEncoder> Asset for TileMap<T, E> {
    const NAME: &'static str = "tiles::map";
    type Data = Self;
    type HandleStorage = HashMapStorage<Handle<Self>>;
}
impl<T: Tile, E: CoordinateEncoder> Component for TileMap<T, E> {
    type Storage = HashMapStorage<Self>;
}

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
impl<T: Tile, E: CoordinateEncoder> TileMap<T, E> {
    /// Versioning for change cache management
    pub fn version(&self) -> u64 {
        self.version
    }

    ///Create a new instance of `TileMap`.
    pub fn new(
        dimensions: Vector3<u32>,
        tile_dimensions: Vector3<u32>,
        sprite_sheet: Option<Handle<SpriteSheet>>,
    ) -> Self {
        let origin = Point3::new(0.0, 0.0, 0.0);
        let transform = create_transform(&dimensions, &tile_dimensions);

        let size = (dimensions.x * dimensions.y * dimensions.z) as usize;
        let mut data = Vec::with_capacity(size);
        data.resize_with(size, T::default);

        let encoder = E::from_dimensions(dimensions.x, dimensions.y, dimensions.z);

        Self {
            data,
            origin,
            dimensions,
            tile_dimensions,
            sprite_sheet,
            transform,
            encoder,
            version: 1,
        }
    }
}

impl<T: Tile, E: CoordinateEncoder> Map for TileMap<T, E> {
    #[inline]
    fn tile_dimensions(&self) -> &Vector3<u32> {
        &self.tile_dimensions
    }

    #[inline]
    fn origin(&self) -> &Point3<f32> {
        &self.origin
    }

    #[inline]
    fn dimensions(&self) -> &Vector3<u32> {
        &self.dimensions
    }

    #[inline]
    fn set_sprite_sheet(&mut self, sprite_sheet: Option<Handle<SpriteSheet>>) {
        self.sprite_sheet = sprite_sheet;
    }

    #[inline]
    fn to_world(&self, coord: &Point3<u32>) -> Vector3<f32> {
        to_world(&self.transform, coord)
    }

    #[inline]
    #[allow(clippy::let_and_return)]
    fn to_tile(&self, coord: &Vector3<f32>) -> Option<Point3<u32>> {
        let ret = to_tile(&self.transform, coord);
        #[cfg(debug_assertions)]
        {
            if let Some(r) = ret.as_ref() {
                if r.x > self.dimensions().x
                    || r.y > self.dimensions().y
                    || r.z > self.dimensions().z
                {
                    panic!(
                    "Requested coordinate is outside map dimensions: '{:?}', max dimensions=:{:?}",
                    *r,
                    self.dimensions()
                );
                }
            }
        }
        ret
    }

    #[inline]
    fn transform(&self) -> &Matrix4<f32> {
        &self.transform
    }

    #[inline]
    fn encode(&self, coord: &Point3<u32>) -> Option<u32> {
        self.encode_raw(&(coord.x, coord.y, coord.z))
    }

    #[inline]
    fn encode_raw(&self, coord: &(u32, u32, u32)) -> Option<u32> {
        self.encoder.encode(coord.0, coord.1, coord.2)
    }

    #[inline]
    fn decode(&self, morton: u32) -> Option<Point3<u32>> {
        let coords = self.encoder.decode(morton)?;
        Some(Point3::new(coords.0, coords.1, coords.2))
    }

    #[inline]
    fn decode_raw(&self, morton: u32) -> Option<(u32, u32, u32)> {
        self.encoder.decode(morton)
    }
}
impl<T: Tile, E: CoordinateEncoder> MapStorage<T> for TileMap<T, E> {
    #[inline]
    fn get(&self, coord: &Point3<u32>) -> Option<&T> {
        self.get_raw(self.encode(coord)?)
    }

    #[inline]
    fn get_mut(&mut self, coord: &Point3<u32>) -> Option<&mut T> {
        self.get_raw_mut(self.encode(coord)?)
    }

    #[inline]
    fn get_mut_nochange(&mut self, coord: &Point3<u32>) -> Option<&mut T> {
        self.get_raw_mut_nochange(self.encode(coord)?)
    }

    #[inline]
    fn get_raw(&self, coord: u32) -> Option<&T> {
        #[cfg(debug_assertions)]
        {
            if coord > self.encode(&Point3::from(*self.dimensions()))? {
                return None;
            }
        }

        self.data.get(coord as usize)
    }

    #[inline]
    fn get_raw_mut(&mut self, coord: u32) -> Option<&mut T> {
        self.version += 1;
        self.data.get_mut(coord as usize)
    }

    #[inline]
    fn get_raw_mut_nochange(&mut self, coord: u32) -> Option<&mut T> {
        self.data.get_mut(coord as usize)
    }
}

#[allow(clippy::cast_precision_loss)]
fn create_transform(map_dimensions: &Vector3<u32>, tile_dimensions: &Vector3<u32>) -> Matrix4<f32> {
    let tile_dimensions = Vector3::new(
        tile_dimensions.x as f32,
        tile_dimensions.y as f32,
        tile_dimensions.z as f32,
    );

    let half_dimensions = Vector3::new(
        -1.0 * (map_dimensions.x as f32 / 2.0),
        map_dimensions.y as f32 / 2.0,
        0.0,
    );

    Matrix4::new_translation(&half_dimensions).append_nonuniform_scaling(&tile_dimensions)
}

#[allow(clippy::cast_precision_loss)]
fn to_world(transform: &Matrix4<f32>, coord: &Point3<u32>) -> Vector3<f32> {
    let coord_f = Point3::new(coord.x as f32, -1.0 * coord.y as f32, coord.z as f32);
    transform.transform_point(&coord_f).coords
}

fn to_tile(transform: &Matrix4<f32>, coord: &Vector3<f32>) -> Option<Point3<u32>> {
    let point = Point3::from(*coord);

    let mut inverse = transform
        .try_inverse()
        .unwrap()
        .transform_point(&point)
        .coords;

    inverse.x = inverse.x.round();
    inverse.y = inverse.y.round() * -1.0;
    inverse.z = inverse.z.floor();

    if inverse.x < 0.0 {
        return None;
    }
    if inverse.y < 0.0 {
        return None;
    }
    if inverse.z < 0.0 {
        return None;
    }

    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    Some(Point3::new(
        inverse.x as u32,
        inverse.y as u32,
        inverse.z as u32,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use amethyst_core::math::Point3;

    pub fn test_coord(transform: &Matrix4<f32>, tile: Point3<u32>, world: Point3<f32>) {
        let world_result = to_world(transform, &tile);
        assert_eq!(world_result, world.coords);
        let tile_result = to_tile(transform, &world.coords).unwrap();
        assert_eq!(tile_result, tile);

        let world_reverse = to_tile(transform, &world_result).unwrap();
        assert_eq!(world_reverse, tile);
        let tile_reverse = to_world(transform, &tile_result);
        assert_eq!(tile_reverse, world.coords);
    }

    #[test]
    pub fn tilemap_coord_conversions() {
        let transform = create_transform(&Vector3::new(64, 64, 64), &Vector3::new(10, 10, 1));

        test_coord(
            &transform,
            Point3::new(0, 0, 0),
            Point3::new(-320.0, 320.0, 0.0),
        );
        test_coord(
            &transform,
            Point3::new(1, 0, 0),
            Point3::new(-310.0, 320.0, 0.0),
        );
        test_coord(
            &transform,
            Point3::new(0, 1, 0),
            Point3::new(-320.0, 310.0, 0.0),
        );

        test_coord(
            &transform,
            Point3::new(0, 1, 20),
            Point3::new(-320.0, 310.0, 20.0),
        );
    }
}
