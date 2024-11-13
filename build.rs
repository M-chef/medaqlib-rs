// fn main() {
    // println!("cargo:rustc-link-search=native=.");
    // println!("cargo:rustc-link-lib=dylib=MEDAQLib.dll");
// }

use std::env;
use std::path::PathBuf;

use bindgen::builder;

fn main() {

    // Configure and generate bindings.
    let bindings = builder()
        .header("MEDAQLib.h")
        // .allowlist_type("SomeCoolClass")
        // .allowlist_function("do_some_cool_thing")
        .generate()
        .expect("Unable to generate bindings");

    // Write the generated bindings to an output file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
    
    // Tell cargo to link the DLL
    println!("cargo:rustc-link-lib=dylib=MEDAQLib");
    
    // Specify where to find the DLL
    println!("cargo:rustc-link-search=native=.");
}
