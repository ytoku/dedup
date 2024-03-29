use std::clone::Clone;
use std::cmp::{Eq, PartialEq};
use std::collections::{HashMap, HashSet};
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
    pub fn new(mtime: FileTime, nlink: u64, realsize: u64) -> Self {
        Self {
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
    pub fn new() -> Self {
        Self {
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
pub struct IdenticalFile {
    pub inos: Vec<Ino>,
}

impl IdenticalFile {
    pub fn new() -> Self {
        Self { inos: Vec::new() }
    }
}

#[derive(Debug)]
pub struct IdenticalFiles {
    pub map: HashMap<Sha256Value, IdenticalFile>,
}

impl IdenticalFiles {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn get_or_insert(&mut self, hash: Sha256Value) -> &mut IdenticalFile {
        self.map.entry(hash).or_insert_with(IdenticalFile::new)
    }
}

#[derive(Debug)]
pub enum FileSizeSieveEntry {
    Unique(Ino),
    Ambiguous,
}

#[derive(Debug)]
pub struct FileSizeSieve {
    map: HashMap<u64, FileSizeSieveEntry>,
}

impl FileSizeSieve {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn get_mut(&mut self, size: u64) -> Option<&mut FileSizeSieveEntry> {
        self.map.get_mut(&size)
    }

    pub fn set_unique(&mut self, size: u64, ino: Ino) {
        self.map.insert(size, FileSizeSieveEntry::Unique(ino));
    }
}

#[derive(Debug)]
pub struct VisitedDirs {
    pub set: HashSet<Ino>,
}

impl VisitedDirs {
    pub fn new() -> Self {
        Self {
            set: HashSet::new(),
        }
    }

    pub fn visit(&mut self, ino: Ino) -> bool {
        self.set.insert(ino)
    }
}

#[derive(Debug)]
pub struct Device {
    pub inodes: Inodes,
    pub sieve: FileSizeSieve,
    pub identicals: IdenticalFiles,
    pub visited_dirs: VisitedDirs,
}

impl Device {
    pub fn new() -> Self {
        Self {
            inodes: Inodes::new(),
            sieve: FileSizeSieve::new(),
            identicals: IdenticalFiles::new(),
            visited_dirs: VisitedDirs::new(),
        }
    }
}

#[derive(Debug)]
pub struct Database {
    pub devices: HashMap<Dev, Device>,
}

impl Database {
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
        }
    }

    pub fn get_or_insert(&mut self, dev: Dev) -> &mut Device {
        self.devices.entry(dev).or_insert_with(Device::new)
    }
}
