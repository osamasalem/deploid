use std::error::Error;

use winres;

fn main() -> Result<(), Box<dyn Error>> {
    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("..\\deploid.ico")
            .set("InternalName", "Deploid.exe")
            .set_version_info(winres::VersionInfo::PRODUCTVERSION, 0x0001000000000000);
        res.compile()?;
    }
    Ok(())
}
