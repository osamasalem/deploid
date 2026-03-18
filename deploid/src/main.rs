use std::{
    error::Error,
    fs::{self, File},
    io::{self, BufReader, BufWriter, Seek, Write},
    path::{Path, PathBuf},
};

const DEPLOID: &str = r#"
░███████   ░██████████ ░█████████  ░██           ░██████   ░██████░███████
░██   ░██  ░██         ░██     ░██ ░██          ░██   ░██    ░██  ░██   ░██
░██    ░██ ░██         ░██     ░██ ░██         ░██     ░██   ░██  ░██    ░██
░██    ░██ ░█████████  ░█████████  ░██         ░██     ░██   ░██  ░██    ░██
░██    ░██ ░██         ░██         ░██         ░██     ░██   ░██  ░██    ░██
░██   ░██  ░██         ░██         ░██          ░██   ░██    ░██  ░██   ░██
░███████   ░██████████ ░██         ░██████████   ░██████   ░██████░███████
"#;

#[cfg(debug_assertions)]
const INSTALLER_EXEC: &[u8] = include_bytes!("..\\..\\target\\debug\\installer.exe");

#[cfg(not(debug_assertions))]
const INSTALLER_EXEC: &[u8] = include_bytes!("..\\..\\target\\release\\installer.exe");

use clap::Parser;
use crossterm::style::Stylize;
use wincode::{self, SchemaRead, SchemaWrite};
use zip::write::FileOptions;

const MAGIC_NUM: u64 = 0xFEEDCAFEBABEFACE;

#[derive(SchemaWrite, SchemaRead)]
struct PostExecutableHeader {
    magic: u64,
    size: usize,
}

impl PostExecutableHeader {
    fn new(size: usize) -> Self {
        Self {
            magic: MAGIC_NUM,
            size,
        }
    }
}

fn create_zip_from_folder(
    source: &mut (impl Write + Seek),
    path: &Path,
) -> Result<(), Box<dyn Error>> {
    let mut zip = zip::ZipWriter::new(source);

    if !path.is_dir() {
        return Err("Invalid path".into());
    }
    let pack = PathBuf::new();

    let mut count = 0;
    for entry in walkdir::WalkDir::new(path) {
        let entry = entry?;
        let mut entrypath = pack.clone();
        entrypath.push(entry.path().strip_prefix(path)?);
        if entry.file_type().is_file() {
            println!(
                "{} {} {}",
                "*".red(),
                "Adding file:".grey(),
                entrypath.display().to_string().green()
            );
            zip.start_file_from_path(entrypath, FileOptions::DEFAULT)?;
            let file = File::open(entry.path())?;
            let mut file = BufReader::new(file);
            io::copy(&mut file, &mut zip)?;
            count += 1
        }
    }

    zip.finish()?;

    println!(
        "{} {} {} {}",
        "-".cyan(),
        "Total".yellow(),
        count.to_string().blue(),
        "file(s) written..".yellow()
    );

    Ok(())
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct CommandLineArgs {
    #[arg(
        short,
        long,
        value_name = "FOLDER",
        help = "Source folder where installation files lie."
    )]
    source: String,

    #[arg(
        short,
        long,
        value_name = "PATH",
        default_value = ".\\install.exe",
        help = "the output file path"
    )]
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = CommandLineArgs::parse();

    println!("{}", DEPLOID.cyan());

    println!("{} {}", "-".cyan(), "Command line parsed..".yellow());

    println!(
        "{} {}{}",
        "-".cyan(),
        "Starting file :".yellow(),
        cli.output.display().to_string().green()
    );

    let file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(cli.output)?;
    let mut file = BufWriter::new(file);

    println!(
        "{} {}",
        "-".cyan(),
        "Writing the runner executable..".yellow()
    );

    file.write_all(INSTALLER_EXEC)?;

    println!(
        "{} {}",
        "-".cyan(),
        "Start adding and compressing package files..".yellow()
    );

    let start = INSTALLER_EXEC.len() as u64;

    create_zip_from_folder(&mut file, &Path::new(&cli.source))?;
    let size = file.stream_position()? - start;

    println!("{} {}", "-".cyan(), "Writing footer structure..".yellow());

    let header = PostExecutableHeader::new(size as usize);
    let header = wincode::serialize(&header)?;

    file.write_all(&header)?;
    println!("{} {}", "-".cyan(), "Done..".yellow());

    Ok(())
}
