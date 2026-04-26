use std::{borrow::Cow, error::Error, path::Path};

use dm3dm4::dataset::{DMArray, DMDataSet};
use mrc::MrcMmap;

use crate::read::Volume3D;

pub struct MrcDataSource {
    pub src: MrcMmap,
}

pub struct DmDataSource {
    pub src: DMArray,
}

pub trait DataSource: Sync + Send {
    fn dimensions(&self) -> (usize, usize, usize);
    fn get_slice<'a>(&'a self, z: usize) -> Cow<'a, [i16]>;
}

impl DataSource for MrcDataSource {
    fn dimensions(&self) -> (usize, usize, usize) {
        let read_view = self.src.read_view().unwrap();
        read_view.dimensions()
    }

    fn get_slice(&self, z: usize) -> Cow<'_, [i16]> {
        let read_view = self.src.read_view().unwrap();
        let volume = Volume3D::new(read_view);
        Cow::Borrowed(&volume.get_slice(z).unwrap())
    }
}

impl DataSource for DmDataSource {
    fn dimensions(&self) -> (usize, usize, usize) {
        let raw_shape = self.src.shape();
        (raw_shape[0], raw_shape[1], raw_shape[2])
    }

    fn get_slice(&self, z: usize) -> Cow<'_, [i16]> {
        self.src.as_zslice::<i16>(z).unwrap()
    }
}

pub fn load_any(path: &Path) -> Result<Box<dyn DataSource>, Box<dyn Error + Sync + Send>> {
    let ext = path.extension().unwrap().to_str().unwrap().to_lowercase();
    let data: Box<dyn DataSource> = if ext == "dm3" || ext == "dm4" {
        let ds = DMDataSet::load(path)?;
        let arrs = ds.arrays()?;
        let arr = arrs.first().unwrap();
        Box::new(DmDataSource { src: arr.clone() })
    } else {
        Box::new(MrcDataSource {
            src: MrcMmap::open(path)?,
        })
    };
    Ok(data)
}
