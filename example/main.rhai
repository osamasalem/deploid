/* The metadata bag the contains the configurations
 * that will be used by the script to be used by the engine
 * or used by this script to be used in different places
 * and branching
 */
fn get_metadata(config) {
    config += #{
        app_name: "Test",
        path: "C:\\TestDeploid\\" ,
        license: load_text_from_file("LICENSE"),
    };
    print(config)
}


/* This function controls the sequence and order of the
 * wizard steps from start to finish, this is called before
 * each step to route the wizard to the proper steps based on
 * configuration
 */
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

/* this is the actual installation script steps
 * this can be controlled by the config that are filled
 * throughout the wizard steps, this function is exactly
 * called in the start of the copy files step
 */
fn installation_actions(config) {
    InstallAction::CopyDir("a","z")
     + InstallAction::CopyDir("b","b")
     +
    [
        InstallAction::CopyFile("LICENSE"),
        InstallAction::CopyFile("README.md"),
        InstallAction::ExecuteCommand("wt.exe",["cmd.exe","/C","dir"]),
    ]

}
