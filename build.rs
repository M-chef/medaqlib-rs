fn main() {
    // Tell cargo to link the DLL
    // println!("cargo:rustc-link-lib=dylib=MEDAQLib");
    
    // Specify where to find the DLL
    println!("cargo:rustc-link-search=native=.");
}
