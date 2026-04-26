
#[derive(Debug, clap::ValueEnum, Clone, Copy)]
pub enum OutputEndianess {
    Big,
    Little,
    Native,
}

impl OutputEndianess {
    /// Constructor: byte order of the current system
    pub fn system() -> Self {
        if cfg!(target_endian = "big") {
            Self::Big
        } else {
            Self::Little
        }
    }

    pub fn for_encoding(&self) -> tiff_encoder::write::Endianness {
        match &self {
            OutputEndianess::Big => tiff_encoder::write::Endianness::MM,
            OutputEndianess::Little => tiff_encoder::write::Endianness::II,
            OutputEndianess::Native => Self::system().for_encoding(),
        }
    }
}
