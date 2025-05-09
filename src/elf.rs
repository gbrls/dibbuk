use clap::builder::Str;
use elf;
use elf::endian;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use thiserror::Error;

use std::io;

#[derive(Debug, Error)]
pub enum DibbukError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("ELF parsing error: {0}")]
    ElfParsing(String),
}

#[derive(Debug)]
pub struct Elf {
    pub inner: elf::ElfBytes<'static, endian::AnyEndian>,
    pub data: Rc<[u8]>,
    pub symbols: HashMap<u64, String>,
}

impl Elf {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, DibbukError> {
        let data = Rc::from(std::fs::read(path)?.into_boxed_slice());

        let data_ref = unsafe { std::mem::transmute::<&[u8], &'static [u8]>(&data) };

        let inner = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(data_ref).unwrap();
        let symbols = Elf::populate_symbols_alt(&inner);

        Ok(Self {
            inner,
            data,
            symbols,
        })
    }

    fn populate_symbols_alt(elf: &elf::ElfBytes<endian::AnyEndian>) -> HashMap<u64, String> {
        if !(elf.symbol_table().is_ok() && elf.symbol_table().unwrap().is_some()) {
            return HashMap::new();
        }

        // FIXME: we need to parse plt entries to get call symbols to there
        let plt = elf.section_header_by_name(".plt").unwrap();

        let sym = elf.symbol_table().unwrap();
        let (symt, strt) = sym.unwrap();
        let sym_map: HashMap<_, _> = symt
            .iter()
            .filter_map(|symbol| {
                if symbol.st_name > 0 {
                    strt.get(symbol.st_name as usize)
                        .ok()
                        .and_then(|name| Some((symbol.st_value, name.to_string())))
                    // WARN: this st_value might not be the correct way to get the addr
                } else {
                    None
                }
            })
            .collect();

        //let sym = elf.dynamic_symbol_table().unwrap();
        //let (symt, strt) = sym.unwrap();
        //let dysym_map = symt.iter().filter_map(|symbol| {
        //    if symbol.st_name > 0 {
        //        strt.get(symbol.st_name as usize).ok().and_then(|name| {
        //            println!("{}", name);
        //            Some((symbol.st_value, name.to_string()))
        //        })
        //    } else {
        //        None
        //    }
        //});

        //sym_map.extend(dysym_map);
        sym_map
    }

    //fn populate_symbols(elf: &goblin::elf::Elf) -> HashMap<u64, String> {
    //    let mut mp = HashMap::new();

    //    let syms = elf.syms.to_vec();
    //    let syms_tab = elf.strtab.to_vec().unwrap();

    //    for sym in syms {
    //        let idx = sym.st_name;
    //        if idx < syms_tab.len() {
    //            mp.insert(sym.st_value as u64, syms_tab[idx].into());
    //        }
    //    }

    //    let syms = elf.dynsyms.to_vec();

    //    for sym in syms {
    //        let idx = sym.st_name;
    //        if idx < syms_tab.len() {
    //            mp.insert(sym.st_value as u64, syms_tab[idx].into());
    //        }
    //    }

    //    mp
    //}

    fn got_iter(&self) {
        let elf = &self.inner;
        //elf
    }

    pub fn got(entry: &str) {}
}

mod test {
    use super::Elf;

    //#[test]
    //fn elf_ropemporium_pivot() {
    //    let handle = Elf::new("/home/gbrls/ctf/rop_emporium/dir_pivot/pivot").unwrap();
    //    let syms = handle.inner.syms.to_vec();
    //    let syms_tab = handle.inner.strtab.to_vec().unwrap();

    //    for sym in syms {
    //        let idx = sym.st_shndx;
    //        if idx < syms_tab.len() {
    //            println!("{:#018x} {}", sym.st_value, syms_tab[idx]);
    //        }
    //    }

    //    let syms = handle.inner.dynsyms.to_vec();
    //    //let syms_tab = handle.inner.dynstrtab.to_vec().unwrap();

    //    for sym in syms {
    //        let idx = sym.st_shndx;
    //        if idx < syms_tab.len() {
    //            println!("{:#018x} {}", sym.st_value, syms_tab[idx]);
    //        }
    //    }
    //}

    #[test]
    fn elf_ropemporium_pivot_alt() {
        let data = std::rc::Rc::from(
            std::fs::read("/home/gbrls/ctf/rop_emporium/dir_pivot/pivot")
                .unwrap()
                .into_boxed_slice(),
        );

        let data_ref = unsafe { std::mem::transmute::<&[u8], &'static [u8]>(&data) };

        let elf_in = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(data_ref).unwrap();
        let syms = Elf::populate_symbols_alt(&elf_in);

        syms.iter().for_each(|(addr, name)| {
            println!("{:#018x} {}", addr, name);
        });
    }
}
