use std::fs;
use std::io;
use std::io::prelude::*;
use std::path::Path;

use generic_array::typenum::U32;
use generic_array::GenericArray;
use sha2::{Digest, Sha256};

pub type Sha256Value = GenericArray<u8, U32>;

pub fn sha256file(path: &Path) -> io::Result<Sha256Value> {
    let mut hasher = Sha256::new();
    let file = fs::File::open(path)?;

    const BUFFER_SIZE: usize = 65536;
    let mut reader = io::BufReader::with_capacity(BUFFER_SIZE, file);

    // There is no way to use uninitialized read buffer in stable rust 1.65.
    // Nightly rust has std::io::BorrowedBuf for this purpose.
    const CHUNK_SIZE: usize = 1024;
    let mut chunk = [0_u8; CHUNK_SIZE];

    loop {
        let n = reader.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        hasher.update(&chunk[..n]);
    }
    Ok(hasher.finalize())
}
