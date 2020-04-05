use gimli::write::*;
use gimli::*;

#[derive(Clone)]
pub struct DwarfReloc {
    pub offset: usize,
    pub size: u8,
    pub symbol: usize,
}

#[derive(Clone)]
pub struct DwarfSelfReloc {
    pub offset: usize,
    pub size: u8,
    pub section: &'static str,
}

#[derive(Clone)]
pub struct DwarfWriter {
    inner: EndianVec<LittleEndian>,
    relocs: Vec<DwarfReloc>,
    self_relocs: Vec<DwarfSelfReloc>,
}

impl DwarfWriter {
    pub fn new() -> DwarfWriter {
        DwarfWriter {
            inner: EndianVec::new(gimli::LittleEndian),
            relocs: vec![],
            self_relocs: vec![],
        }
    }

    pub fn take(&mut self) -> (Vec<u8>, Vec<DwarfReloc>, Vec<DwarfSelfReloc>) {
        (
            self.inner.take(),
            std::mem::replace(&mut self.relocs, vec![]),
            std::mem::replace(&mut self.self_relocs, vec![]),
        )
    }
}

impl Writer for DwarfWriter {
    type Endian = LittleEndian;
    fn endian(&self) -> Self::Endian {
        LittleEndian
    }
    fn len(&self) -> usize {
        self.inner.len()
    }
    fn write(&mut self, bytes: &[u8]) -> gimli::write::Result<()> {
        self.inner.write(bytes)
    }
    fn write_at(&mut self, offset: usize, bytes: &[u8]) -> gimli::write::Result<()> {
        self.inner.write_at(offset, bytes)
    }

    fn write_address(&mut self, address: Address, size: u8) -> gimli::write::Result<()> {
        match address {
            Address::Symbol { symbol, addend } => {
                self.relocs.push(DwarfReloc {
                    offset: self.inner.len(),
                    size,
                    symbol,
                });
                self.inner
                    .write_address(Address::Constant(addend as u64), size)
            }
            _ => self.inner.write_address(address, size),
        }
    }

    fn write_eh_pointer(
        &mut self,
        address: Address,
        eh_pe: constants::DwEhPe,
        size: u8,
    ) -> gimli::write::Result<()> {
        match address {
            Address::Symbol { symbol, addend } => {
                self.relocs.push(DwarfReloc {
                    offset: self.inner.len(),
                    size,
                    symbol,
                });
                self.inner
                    .write_eh_pointer(Address::Constant(addend as u64), eh_pe, size)
            }
            _ => self.inner.write_eh_pointer(address, eh_pe, size),
        }
    }

    /// Write an offset that is relative to the start of the given section.
    ///
    /// If the writer supports relocations, then it must provide its own implementation
    /// of this method.
    fn write_offset(
        &mut self,
        val: usize,
        section: SectionId,
        size: u8,
    ) -> gimli::write::Result<()> {
        self.self_relocs.push(DwarfSelfReloc {
            offset: self.inner.len(),
            size,
            section: section.name(),
        });
        self.inner.write_offset(val, section, size)
    }

    /// Write an offset that is relative to the start of the given section.
    ///
    /// If the writer supports relocations, then it must provide its own implementation
    /// of this method.
    fn write_offset_at(
        &mut self,
        offset: usize,
        val: usize,
        section: SectionId,
        size: u8,
    ) -> gimli::write::Result<()> {
        self.self_relocs.push(DwarfSelfReloc {
            offset,
            size,
            section: section.name(),
        });
        self.inner.write_offset_at(offset, val, section, size)
    }
}
