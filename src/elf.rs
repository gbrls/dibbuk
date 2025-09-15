use elf;
use elf::endian;
use elf::relocation::Rela;
use std::collections::HashMap;
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
        let symbols = Elf::read_symbols(&inner);

        Ok(Self {
            inner,
            data,
            symbols,
        })
    }

    fn read_symbols(elf: &elf::ElfBytes<endian::AnyEndian>) -> HashMap<u64, String> {
        let mut sym_map = HashMap::new();

        // Normal symbols
        if let Ok(Some((symt, strt))) = elf.symbol_table() {
            for sym in symt.iter() {
                if sym.st_name > 0 {
                    if let Ok(name) = strt.get(sym.st_name as usize) {
                        sym_map.insert(sym.st_value, name.to_string());
                    }
                }
            }
        }

        // Synthetic @plt symbols
        if let Ok(Some(plt)) = elf.section_header_by_name(".plt") {
            let plt_addr = plt.sh_addr;
            let plt_entsize = plt.sh_entsize.max(16); // usually 16 on x86_64

            if let Ok(Some(rela_plt)) = elf.section_header_by_name(".rela.plt") {
                if let Ok(Some((dynsyms, dynstrs))) = elf.dynamic_symbol_table() {
                    if let Ok(rela_iter) = elf.section_data_as_relas(&rela_plt) {
                        for (i, rela) in rela_iter.enumerate() {
                            let sym_idx = rela.r_sym;
                            if let Ok(sym) = dynsyms.get(sym_idx as usize) {
                                if sym.st_name > 0 {
                                    if let Ok(name) = dynstrs.get(sym.st_name as usize) {
                                        // Skip PLT[0], so offset by +1
                                        let addr = plt_addr + (i as u64 + 1) * plt_entsize;
                                        sym_map.insert(addr, format!("{}@plt", name));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

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
    use crate::elf::Elf;

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
        let syms = Elf::read_symbols(&elf_in);

        syms.iter().for_each(|(addr, name)| {
            println!("{:#018x} {}", addr, name);
        });
    }
    #[test]
    fn elf_frog_alt() {
        let data = std::rc::Rc::from(
            std::fs::read("./resources/frog")
                .unwrap()
                .into_boxed_slice(),
        );

        let data_ref = unsafe { std::mem::transmute::<&[u8], &'static [u8]>(&data) };

        let elf_in = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(data_ref).unwrap();
        let syms = Elf::read_symbols(&elf_in);

        syms.iter().for_each(|(addr, name)| {
            println!("{:#018x} {}", addr, name);
        });
    }
}
