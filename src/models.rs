use std::clone::Clone;
use std::cmp::{Eq, PartialEq};
use std::collections::HashMap;
use std::hash::Hash;
use std::marker::Copy;
use std::path::PathBuf;

use filetime::FileTime;

use crate::digest::Sha256Value;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Ino(pub u64);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Dev(pub u64);

#[derive(Debug)]
pub struct Inode {
    pub mtime: FileTime,
    pub nlink: u64,
    pub realsize: u64,
    pub files: Vec<PathBuf>,
}

impl Inode {
    pub fn new(mtime: FileTime, nlink: u64, realsize: u64) -> Inode {
        Inode {
            mtime,
            nlink,
            realsize,
            files: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct Inodes {
    pub map: HashMap<Ino, Inode>,
}

impl Inodes {
    pub fn new() -> Inodes {
        Inodes {
            map: HashMap::new(),
        }
    }

    pub fn get_or_insert(
        &mut self,
        ino: Ino,
        mtime: FileTime,
        nlink: u64,
        realsize: u64,
    ) -> &mut Inode {
        self.map
            .entry(ino)
            .or_insert_with(|| Inode::new(mtime, nlink, realsize))
    }

    pub fn get(&self, ino: Ino) -> Option<&Inode> {
        self.map.get(&ino)
    }

    pub fn get_mut(&mut self, ino: Ino) -> Option<&mut Inode> {
        self.map.get_mut(&ino)
    }
}

#[derive(Debug)]
#[warn(clippy::new_without_default)]
pub struct IdenticalFile {
    pub inos: Vec<Ino>,
}

impl IdenticalFile {
    pub fn new() -> IdenticalFile {
        IdenticalFile { inos: Vec::new() }
    }
}

#[derive(Debug)]
#[warn(clippy::new_without_default)]
pub struct IdenticalFiles {
    pub map: HashMap<Sha256Value, IdenticalFile>,
}

impl IdenticalFiles {
    pub fn new() -> IdenticalFiles {
        IdenticalFiles {
            map: HashMap::new(),
        }
    }

    pub fn get_or_insert(&mut self, hash: Sha256Value) -> &mut IdenticalFile {
        self.map.entry(hash).or_insert_with(IdenticalFile::new)
    }
}

#[derive(Debug)]
#[warn(clippy::new_without_default)]
pub struct Device {
    pub inodes: Inodes,
    pub identicals: IdenticalFiles,
}

impl Device {
    pub fn new() -> Device {
        Device {
            inodes: Inodes::new(),
            identicals: IdenticalFiles::new(),
        }
    }
}

#[derive(Debug)]
#[warn(clippy::new_without_default)]
pub struct Database {
    pub devices: HashMap<Dev, Device>,
}

impl Database {
    pub fn new() -> Database {
        Database {
            devices: HashMap::new(),
        }
    }

    pub fn get_or_insert(&mut self, dev: Dev) -> &mut Device {
        self.devices.entry(dev).or_insert_with(Device::new)
    }
}
