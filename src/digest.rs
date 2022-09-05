use std::fs;
use std::io;
use std::io::prelude::*;
use std::mem::MaybeUninit;
use std::path::Path;

use generic_array::typenum::U32;
use generic_array::GenericArray;
use sha2::{Digest, Sha256};

pub type Sha256Value = GenericArray<u8, U32>;

pub fn sha256file(path: &Path) -> io::Result<Sha256Value> {
    let mut hasher = Sha256::new();
    let file = fs::File::open(path)?;
    let mut reader = io::BufReader::new(file);

    const CHUNK_SIZE: usize = 65536;
    let mut chunk: [u8; CHUNK_SIZE] = unsafe {
        let uninit: MaybeUninit<[u8; CHUNK_SIZE]> = MaybeUninit::uninit();
        uninit.assume_init()
    };

    loop {
        let n = reader.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        hasher.update(&chunk[..n]);
    }
    Ok(hasher.finalize())
}
