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
}
