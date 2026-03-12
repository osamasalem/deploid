use std::{
    error::Error,
    fs::{self, File, FileType},
    io::{self, BufRead, BufReader, Read, Seek, Write},
    path::{Path, PathBuf},
};

use wincode::{self, SchemaRead, SchemaWrite};
use zip::write::FileOptions;

const MAGIC_NUM: u64 = 0xFEEDCAFEBABEFACE;

#[derive(SchemaWrite, SchemaRead)]
struct PostExecutableHeader {
    magic: u64,
    size: usize,
}

const POST_EXECUTABLE_HEADER_SIZE: usize = size_of::<PostExecutableHeader>();

impl PostExecutableHeader {
    fn new(size: usize) -> Self {
        Self {
            magic: MAGIC_NUM,
            size,
        }
    }
}

fn create_zip_from_folder(file: &mut File, path: &Path) -> Result<(), Box<dyn Error>> {
    let mut zip = zip::ZipWriter::new(file);

    if !path.is_dir() {
        return Err("Invalid path".into());
    }
    let mut pack = PathBuf::new();

    for entry in walkdir::WalkDir::new(path) {
        let entry = entry.unwrap();
        let mut entrypath = pack.clone();
        entrypath.push(entry.path().strip_prefix(path)?);
        if entry.file_type().is_file() {
            println!("FILE : {}", entrypath.display());
            zip.start_file_from_path(entrypath, FileOptions::DEFAULT)?;
            let file = File::open(entry.path())?;
            let mut file = BufReader::new(file);
            io::copy(&mut file, &mut zip)?;
        }
    }

    zip.finish()?;

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    fs::copy("target\\debug\\template.exe", "template.temp.exe")?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .open("template.temp.exe")?;

    file.seek(io::SeekFrom::End(0))?;

    let start = file.stream_position()?;

    create_zip_from_folder(&mut file, &Path::new("installer_pack"))?;
    let size = file.stream_position()? - start;

    dbg!(size);
    let header = PostExecutableHeader::new(size as usize);
    let header = wincode::serialize(&header).unwrap();

    file.write_all(&header).unwrap();
    Ok(())
}
