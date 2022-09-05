use std::cell::RefCell;
use std::clone::Clone;
use std::cmp::{Eq, PartialEq};
use std::collections::HashMap;
use std::hash::Hash;
use std::marker::Copy;
use std::path::PathBuf;
use std::rc::Rc;

use filetime::FileTime;

use crate::digest::Sha256Value;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Ino(pub u64);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Dev(pub u64);

#[derive(Debug)]
pub struct Inode {
    pub mtime: FileTime,
    pub files: Vec<PathBuf>,
}

impl Inode {
    pub fn new(mtime: FileTime) -> Inode {
        Inode {
            mtime,
            files: Vec::new(),
        }
    }
}

#[derive(Debug)]
#[warn(clippy::new_without_default)]
pub struct IdenticalFile {
    pub inodes: HashMap<Ino, Rc<RefCell<Inode>>>,
}

impl IdenticalFile {
    pub fn new() -> IdenticalFile {
        IdenticalFile {
            inodes: HashMap::new(),
        }
    }

    pub fn get_or_insert(&mut self, ino: Ino, mtime: FileTime) -> &mut Rc<RefCell<Inode>> {
        self.inodes
            .entry(ino)
            .or_insert_with(|| Rc::new(RefCell::new(Inode::new(mtime))))
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
    pub identicals: IdenticalFiles,
    pub known_inodes: HashMap<Ino, Rc<RefCell<Inode>>>,
}

impl Device {
    pub fn new() -> Device {
        Device {
            identicals: IdenticalFiles::new(),
            known_inodes: HashMap::new(),
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
