use std::error::Error;

use winres;

fn main() -> Result<(), Box<dyn Error>> {
    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("..\\media\\deploid.ico")
            .set("InternalName", "Deploid.exe");
        res.compile()?;
    }
    Ok(())
}
