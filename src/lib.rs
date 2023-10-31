mod digest;
mod models;

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context as _, Result};
use filetime::FileTime;
use num_format::{Locale, ToFormattedString};
use walkdir::WalkDir;

use crate::digest::sha256file;
use crate::models::*;

#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    targets: Vec<PathBuf>,
}

fn insert_identical_file(identicals: &mut IdenticalFiles, path: &Path, ino: Ino) -> Result<()> {
    let hash = sha256file(path)
        .with_context(|| format!("Failed to calculate a hash: {}", path.to_string_lossy()))?;
    let identical = identicals.get_or_insert(hash);
    identical.inos.push(ino);
    Ok(())
}

fn prepare_file(database: &mut Database, path: &Path, metadata: &fs::Metadata) -> Result<()> {
    let dev = Dev(metadata.dev());
    let ino = Ino(metadata.ino());

    let device = database.get_or_insert(dev);
    if let Some(inode) = device.inodes.get_mut(ino) {
        inode.files.push(path.to_path_buf());
        return Ok(());
    }

    let mtime = FileTime::from_last_modification_time(metadata);

    let nlink = metadata.nlink();
    let realsize = metadata.blocks() * 512;

    let inode = device.inodes.get_or_insert(ino, mtime, nlink, realsize);
    inode.files.push(path.to_path_buf());

    let size = metadata.size();
    match device.sieve.get_mut(size) {
        // first time: mark unique
        None => device.sieve.set_unique(size, ino),
        // already seen
        Some(sieve_entry) => {
            if let &mut FileSizeSieveEntry::Unique(ino0) = sieve_entry {
                // second time: unmark unique and calculate the hash of previous found file
                *sieve_entry = FileSizeSieveEntry::Ambiguous;
                let path0 = &device.inodes.get(ino0).unwrap().files[0];
                insert_identical_file(&mut device.identicals, path0, ino0)?;
            }
            // calculate the hash of current file
            insert_identical_file(&mut device.identicals, path, ino)?;
        }
    }
    Ok(())
}

fn update_mtime<P: AsRef<Path>>(filepath: P, mtime: FileTime) -> Result<()> {
    let metadata = &fs::metadata(&filepath).with_context(|| {
        format!(
            "Failed to fs::metadata: {}",
            filepath.as_ref().to_string_lossy(),
        )
    })?;
    let file_mtime = FileTime::from_last_modification_time(metadata);
    if file_mtime != mtime {
        filetime::set_file_mtime(&filepath, mtime).with_context(|| {
            format!(
                "Failed to filetime::set_file_mtime: {}",
                filepath.as_ref().to_string_lossy(),
            )
        })?;
    }
    Ok(())
}

fn relink<P: AsRef<Path>, Q: AsRef<Path>>(original: P, link: Q) -> Result<()> {
    let link_path = link.as_ref();
    let link_dir_path = link_path.parent().context("Failed to get a parent path")?;
    let original_metadata = fs::metadata(link_dir_path).with_context(|| {
        format!(
            "Failed to fs::metadata for original: {}",
            link_dir_path.to_string_lossy(),
        )
    })?;
    let link_dir_metadata = fs::metadata(link_dir_path).with_context(|| {
        format!(
            "Failed to fs::metadata or link_dir: {}",
            link_dir_path.to_string_lossy(),
        )
    })?;
    ensure!(
        original_metadata.dev() == link_dir_metadata.dev(),
        "dev mismatch",
    );

    let link_dir_mtime = FileTime::from_last_modification_time(&link_dir_metadata);

    fs::remove_file(&link).with_context(|| {
        format!(
            "Failed to fs::remove_file: {}",
            link.as_ref().to_string_lossy(),
        )
    })?;
    fs::hard_link(&original, &link).with_context(|| {
        format!(
            "Failed to fs::hard_link: {}, {}",
            original.as_ref().to_string_lossy(),
            link.as_ref().to_string_lossy(),
        )
    })?;

    filetime::set_file_mtime(link_dir_path, link_dir_mtime).with_context(|| {
        format!(
            "Failed to filetime::set_file_mtime to restore a directory mtime: {}",
            link_dir_path.to_string_lossy(),
        )
    })?;

    Ok(())
}

fn walk_and_prepare(args: &Args, database: &mut Database) -> Result<()> {
    for target in &args.targets {
        let mut it = WalkDir::new(target).into_iter();
        while let Some(entry) = it.next() {
            let entry = entry.context("Failed to get a entry")?;
            let path = &entry.path();
            let metadata = entry
                .metadata()
                .with_context(|| format!("Failed to get metadata: {}", path.to_string_lossy()))?;
            if metadata.is_dir() {
                let dev = Dev(metadata.dev());
                let ino = Ino(metadata.ino());
                // If the directory is already visited, do not walk into the directory.
                // For example:
                // - duplicated targets
                // - bind mount
                if !database.get_or_insert(dev).visited_dirs.visit(ino) {
                    it.skip_current_dir();
                }
            } else if metadata.is_file() {
                prepare_file(database, path, &metadata)?;
            }
        }
    }
    Ok(())
}

fn execute_relink(database: &Database) -> Result<u64> {
    let mut gain: u64 = 0;
    for device in database.devices.values() {
        for identical in device.identicals.map.values() {
            let mut inodes: Vec<_> = identical
                .inos
                .iter()
                .map(|&ino| device.inodes.get(ino).unwrap())
                .collect();
            inodes.sort_by_key(|inode| std::cmp::Reverse(inode.nlink));

            if inodes.len() <= 1 {
                continue;
            }

            let original_path = inodes[0].files[0].as_path();
            println!("{}", &original_path.display());

            let mtime = inodes.iter().map(|inode| inode.mtime).min().unwrap();
            update_mtime(original_path, mtime)?;

            for &inode in &inodes[1..] {
                for filepath in &inode.files {
                    println!("<- {}", &filepath.display());
                    relink(original_path, filepath)?;
                }
                if inode.files.len() as u64 == inode.nlink {
                    gain += inode.realsize;
                }
            }
        }
    }
    Ok(gain)
}

pub fn run(args: Args) -> Result<()> {
    let mut database = Database::new();
    walk_and_prepare(&args, &mut database)?;
    let gain = execute_relink(&database)?;
    println!("Gain: {} bytes", gain.to_formatted_string(&Locale::en));
    Ok(())
}
