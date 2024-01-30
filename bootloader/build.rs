use std::{env, path::Path};

fn main () {
    let kernel_path = match env::var("KERNEL_PATH") {
        Ok(path) => path,
        Err(_) => {
            println!("cargo:warning=Kernel binary path must be set in the \"KERNEL_PATH\" env variable");
            return;
        }
    };

    // link kernel
    println!("cargo:rustc-link-search=native={}", kernel_path);
    println!("cargo:rustc-link-lib=static=kernel");

    // rebuild if new kernel
    println!("cargo:rerun-if-changed={}", Path::new(&kernel_path).join("libkernel.a").to_string_lossy());
    println!("cargo:rerun-if-env-changed=KERNEL_PATH");
}
