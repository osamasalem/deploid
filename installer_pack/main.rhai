

fn get_metadata(config) {

    config += #{
        app_name: "Test",
        path: #"installpath"# ,
    };
}


fn next_action(config, action) {

    switch(action.enum_type) {
        "builtin_init" => Step::Welcome,
        "builtin_welcome" => Step::Path,
//      "builtin_license" => Step::Path,
        "builtin_path" => Step::Confirm,
    }
}


fn installation_actions(config) {
    let pack = InstallAction::GenerateCopyDirToEntries("contrib","contrib");
    pack += InstallAction::GenerateCopyDirToEntries("runtime","runtime");
    pack += [
        InstallAction::CopyFile("hx.exe"),
        InstallAction::CopyFile("LICENSE"),
        InstallAction::CopyFile("README.md"),
    ];
    pack
}
