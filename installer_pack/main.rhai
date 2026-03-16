

fn get_metadata(config) {


    config += #{
        app_name: "Test",
        path: #"installpath"# ,
        license: "LICENSE",
    };
    print(config)
}


fn next_action(config, action) {

    switch(action.enum_type) {
        "builtin_init"      => Step::Welcome,
        "builtin_welcome"   => Step::License,
        "builtin_license"   => Step::Path,
        "builtin_path"      => Step::Confirm,
        "builtin_confirm"   => Step::CopyFiles,
        "builtin_copyfiles" => Step::Finish
    }
}


fn installation_actions(config) {
    InstallAction::CopyDir("contrib","contrib")
    + InstallAction::CopyDir("runtime","runtime")
    + [
        InstallAction::CopyFile("hx.exe"),
        InstallAction::CopyFile("LICENSE"),
        InstallAction::CopyFile("README.md"),
    ];

}
