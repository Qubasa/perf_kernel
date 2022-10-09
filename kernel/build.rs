fn main() {
    use std::env;
    use std::path::PathBuf;

    eprintln!("========= ENV VARS START ===========");
    for (key, value) in env::vars() {
        if key.starts_with("CARGO")
            || key == "OUT_DIR"
            || key == "DEBUG"
            || key == "PROFILE"
            || key == "TARGET"
        {
            eprint!("{key}: {value}, ");
        }
    }
    eprintln!("\n========= ENV VARS END =============");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("Couldn't find env var OUT_DIR"));
    let profile = env::var("PROFILE").expect("Couldn't find env var PROFILE");

    let out_root: Option<PathBuf> = {
        let mut tmp = None;
        for dir in out_dir.ancestors() {
            if dir.ends_with(profile.as_str()) {
                tmp = Some(PathBuf::from(dir));
            }
        }
        tmp
    };

    println!(
        "glue_gun:out_root={:?}",
        out_root.expect("Couldn't find OUT_ROOT")
    );
    println!("cargo:rustc-link-arg=--image-base=0x200000");
}
