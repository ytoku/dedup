mod digest;
mod models;

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use anyhow::{ensure, Context as _, Result};
use filetime::FileTime;
use num_format::{Locale, ToFormattedString};
use walkdir::WalkDir;

use crate::digest::sha256file;
use crate::models::*;

#[derive(clap::Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    #[clap(value_parser)]
    targets: Vec<String>,
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
    let hash = sha256file(path)
        .with_context(|| format!("Failed to calculate a hash: {}", path.to_string_lossy()))?;

    let nlink = metadata.nlink();
    let realsize = metadata.blocks() * 512;

    let identical = device.identicals.get_or_insert(hash);
    let inode = device.inodes.get_or_insert(ino, mtime, nlink, realsize);
    inode.files.push(path.to_path_buf());
    identical.inos.push(ino);
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
        for entry in WalkDir::new(target) {
            let entry = entry.context("Failed to get a entry")?;
            let path = &entry.path();
            if !entry.file_type().is_file() {
                continue;
            }
            let metadata = entry
                .metadata()
                .with_context(|| format!("Failed to get metadata: {}", path.to_string_lossy()))?;
            prepare_file(database, path, &metadata)?;
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
            inodes.sort_by_key(|inode| std::cmp::Reverse(inode.files.len()));

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
