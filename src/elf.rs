use goblin::{error, Object};
use std::env;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use thiserror::Error;

use std::io;

#[derive(Debug, Error)]
enum DibbukError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("ELF parsing error: {0}")]
    ElfParsing(String),
}

impl From<goblin::error::Error> for DibbukError {
    fn from(err: goblin::error::Error) -> Self {
        DibbukError::ElfParsing(err.to_string())
    }
}

#[derive(Debug)]
struct Elf {
    inner: goblin::elf::Elf<'static>,
    data: Rc<[u8]>,
}

impl Elf {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, DibbukError> {
        let data = Rc::from(std::fs::read(path)?.into_boxed_slice());

        let data_ref = unsafe { std::mem::transmute::<&[u8], &'static [u8]>(&data) };

        let inner = goblin::elf::Elf::parse(data_ref)?;

        Ok(Self { inner, data })
    }
    
    fn got_iter(self: Self) {
        //self.inner.
    }

    pub fn got(entry: &str) {}
}

fn run() -> error::Result<()> {
    for (i, arg) in env::args().enumerate() {
        if i == 1 {
            let path = Path::new(arg.as_str());
            let buffer = fs::read(path)?;
            match Object::parse(&buffer)? {
                Object::Elf(elf) => {
                    println!("elf: {:#?}", &elf);
                }
                Object::PE(pe) => {
                    println!("pe: {:#?}", &pe);
                }
                Object::COFF(coff) => {
                    println!("coff: {:#?}", &coff);
                }
                Object::Mach(mach) => {
                    println!("mach: {:#?}", &mach);
                }
                Object::Archive(archive) => {
                    println!("archive: {:#?}", &archive);
                }
                Object::Unknown(magic) => {
                    println!("unknown magic: {:#x}", magic)
                }
                _ => {}
            }
        }
    }
    Ok(())
}
