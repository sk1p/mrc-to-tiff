use std::borrow::Cow;

use dm3dm4::dataset::DMArray;
use mrc::MrcMmap;

use crate::read::Volume3D;

pub struct MrcDataSource {
    pub src: MrcMmap,
}

pub struct DmDataSource {
    pub src: DMArray,
}

pub trait DataSource {
    fn dimensions(&self) -> (usize, usize, usize);
    fn get_slice(&self, z: usize) -> Cow<'_, [f32]>;
}

impl DataSource for MrcDataSource {
    fn dimensions(&self) -> (usize, usize, usize) {
        let read_view = self.src.read_view().unwrap();
        read_view.dimensions()
    }

    fn get_slice(&self, z: usize) -> Cow<'_, [f32]> {
        let read_view = self.src.read_view().unwrap();
        let volume = Volume3D::new(read_view);
        volume.get_slice(z).unwrap().iter().map(|item| *item as f32).collect()
    }
}

impl DataSource for DmDataSource {
    fn dimensions(&self) -> (usize, usize, usize) {
        let raw_shape = self.src.shape();
        (raw_shape[0], raw_shape[1], raw_shape[2])
    }

    fn get_slice(&self, z: usize) -> Cow<'_, [f32]> {
        self.src.as_zslice::<f32>(z).unwrap()
    }
}
