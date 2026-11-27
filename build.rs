use copy_to_output::copy_to_output;
use std::env;
use wesl::Wesl;

fn copy_assets() {
    println!("cargo:rerun-if-changed=assets/*");
    copy_to_output("assets", &env::var("PROFILE").unwrap()).expect("Could not copy assets");
}

fn compile_shaders() {
    let wesl = Wesl::new("src/shaders");
    wesl.build_artifact(&"package::gbuffer".parse().unwrap(), "gbuffer");
    wesl.build_artifact(&"package::light_segment".parse().unwrap(), "light_segment");
    wesl.build_artifact(&"package::light_draw".parse().unwrap(), "light_draw");
    wesl.build_artifact(&"package::compositor".parse().unwrap(), "compositor");
}

fn main() {
    copy_assets();
    compile_shaders();
}
