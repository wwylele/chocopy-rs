use gimli::{write::*, *};

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
            inner: EndianVec::new(LittleEndian),
            relocs: vec![],
            self_relocs: vec![],
        }
    }

    pub fn take(&mut self) -> (Vec<u8>, Vec<DwarfReloc>, Vec<DwarfSelfReloc>) {
        (
            self.inner.take(),
            std::mem::take(&mut self.relocs),
            std::mem::take(&mut self.self_relocs),
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
    fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.inner.write(bytes)
    }
    fn write_at(&mut self, offset: usize, bytes: &[u8]) -> Result<()> {
        self.inner.write_at(offset, bytes)
    }

    fn write_address(&mut self, address: Address, size: u8) -> Result<()> {
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
    ) -> Result<()> {
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

    fn write_offset(&mut self, val: usize, section: SectionId, size: u8) -> Result<()> {
        self.self_relocs.push(DwarfSelfReloc {
            offset: self.inner.len(),
            size,
            section: section.name(),
        });
        self.inner.write_offset(val, section, size)
    }

    fn write_offset_at(
        &mut self,
        offset: usize,
        val: usize,
        section: SectionId,
        size: u8,
    ) -> Result<()> {
        self.self_relocs.push(DwarfSelfReloc {
            offset,
            size,
            section: section.name(),
        });
        self.inner.write_offset_at(offset, val, section, size)
    }
}
