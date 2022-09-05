mod digest;
mod models;

use std::fs;
use std::os::linux::fs::MetadataExt;
use std::path::Path;

use anyhow::{bail, Context as _, Result};
use filetime::FileTime;
use walkdir::WalkDir;

use crate::digest::sha256file;
use crate::models::*;

#[derive(clap::Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    #[clap(value_parser)]
    targets: Vec<String>,
}

fn prepare_file(database: &mut Database, path: &Path) -> Result<()> {
    let metadata = fs::metadata(&path)
        .with_context(|| format!("Failed to fs::metadata: {}", path.to_string_lossy()))?;
    let dev = Dev(metadata.st_dev());
    let ino = Ino(metadata.st_ino());

    let device = database.get_or_insert(dev);
    if let Some(inode) = device.known_inodes.get(&ino) {
        inode.borrow_mut().files.push(path.to_path_buf());
        return Ok(());
    }

    let mtime = FileTime::from_last_modification_time(&metadata);
    let hash = sha256file(path)
        .with_context(|| format!("Failed to calculate a hash: {}", path.to_string_lossy()))?;

    let identical = device.identicals.get_or_insert(hash);
    let inode = identical.get_or_insert(ino, mtime);
    device.known_inodes.insert(ino, inode.clone());
    inode.borrow_mut().files.push(path.to_path_buf());
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
    if original_metadata.st_dev() != link_dir_metadata.st_dev() {
        bail!("dev mismatch");
    }

    let dir_mtime = FileTime::from_last_modification_time(&link_dir_metadata);

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

    filetime::set_file_mtime(&link_dir_path, dir_mtime).with_context(|| {
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
            let path = entry.context("Failed to get a entry")?.into_path();
            if !path.is_file() {
                continue;
            }
            prepare_file(database, path.as_path())?;
        }
    }
    Ok(())
}

fn execute_relink(database: &Database) -> Result<()> {
    for device in database.devices.values() {
        for identical in device.identicals.map.values() {
            let mut inodes: Vec<_> = identical.inodes.values().collect();
            inodes.sort_by_key(|inode| std::cmp::Reverse(inode.borrow_mut().files.len()));

            let inode_ref = inodes[0].borrow();
            let original_path = inode_ref.files[0].as_path();

            if inodes.len() > 1 {
                println!("{}", &original_path.display());
            }

            let mtime = inodes
                .iter()
                .map(|inode| inode.borrow().mtime)
                .min()
                .unwrap();
            update_mtime(original_path, mtime)?;

            for &inode in inodes.iter().skip(1) {
                for filepath in &inode.borrow().files {
                    println!("<- {}", &filepath.display());
                    relink(original_path, filepath)?;
                }
            }
        }
    }
    Ok(())
}

pub fn run(args: Args) -> Result<()> {
    let mut database = Database::new();
    walk_and_prepare(&args, &mut database)?;
    execute_relink(&database)?;
    Ok(())
}
