use mrc::MrcView;

// adapted from the docs of the mrc crate
pub struct Volume3D<'a> {
    view: MrcView<'a>,
    nx: usize,
    ny: usize,
    nz: usize,
}

impl<'a> Volume3D<'a> {
    pub fn new(view: MrcView<'a>) -> Self {
        let (nx, ny, nz) = view.dimensions();
        Self { view, nx, ny, nz }
    }

    pub fn get_slice(&self, z: usize) -> Result<&[i16], mrc::Error> {
        if z >= self.nz {
            return Err(mrc::Error::InvalidDimensions);
        }

        let slice_size = self.nx * self.ny;
        let start = z * slice_size;
        let ints = self.view.data.as_i16_slice()?;

        ints.get(start..start + slice_size)
            .ok_or(mrc::Error::InvalidDimensions)
    }
}