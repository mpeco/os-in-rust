use std::{path::Path, process::Command};
use llvm_tools::LlvmTools;


fn main() {
    // llvm_tools
    let llvm_tools = LlvmTools::new().expect("llvm-tools rustup component probably missing.");

    let is_release = if cfg!(debug_assertions) { false } else { true };

    // paths
    let kernel_path = Path::new("./kernel").canonicalize().unwrap();
    let kernel_target_path =
        if is_release { Path::new(&kernel_path).join("target/x86_64-kernel-target/release") }
        else { Path::new(&kernel_path).join("target/x86_64-kernel-target/debug") };
    let bootloader_path = Path::new("./bootloader").canonicalize().unwrap();
    let bootloader_target_path = Path::new(&bootloader_path).join("target/x86-bootloader-target/release");
    let llvm_objcopy_path = llvm_tools.tool(&llvm_tools::exe("llvm-objcopy"))
        .expect("llvm-objcopy not found.");
    let llvm_ar_path = llvm_tools.tool(&llvm_tools::exe("llvm-ar"))
        .expect("llvm-ar not found.");

    // cargo command
    let mut cargo_build = Command::new("cargo");
    cargo_build.args(["build"]);
    if is_release { cargo_build.args(["--release"]); }

    // build kernel
    cargo_build.current_dir(&kernel_path);
    assert!(cargo_build.status().unwrap().success(), "Failed to build kernel");
    // strip debug symbols
    let mut llvm_objcopy = Command::new(&llvm_objcopy_path);
    llvm_objcopy.current_dir(&kernel_target_path);
    llvm_objcopy.args(["--strip-debug", "kernel", "kernel-elf"]);
    assert!(llvm_objcopy.status().expect("Failed to execute llvm-objcopy on kernel").success(),
        "Error while running llvm-objcopy on kernel");
    // wrap kernel executable in ELF file with size info
    let mut llvm_objcopy = Command::new(&llvm_objcopy_path);
    llvm_objcopy.current_dir(&kernel_target_path);
    llvm_objcopy.args(["-I", "binary", "-O", "elf32-i386",
        "--rename-section", ".data=.kernel", "kernel-elf", "kernel-wrapped"]);
    assert!(llvm_objcopy.status().expect("Failed to execute llvm-objcopy on kernel").success(),
        "Error while running llvm-objcopy on kernel");
    // turn into archive library for linking
    let mut llvm_ar = Command::new(llvm_ar_path);
    llvm_ar.current_dir(&kernel_target_path);
    llvm_ar.args(["crs", "libkernel.a", "kernel-wrapped"]);
    assert!(llvm_ar.status().expect("Failed to execute llvm-objcopy on kernel").success(),
        "Error while running llvm-ar on kernel");

    // build bootloader
    let mut cargo_build = Command::new("cargo");
    cargo_build.args(["build", "--release"]);
    cargo_build.current_dir(&bootloader_path);
    cargo_build.env("KERNEL_PATH", &kernel_target_path);
    assert!(cargo_build.status().unwrap().success(), "Failed to build bootloader");
    // objcopy bootloader into raw binary
    let mut llvm_objcopy = Command::new(&llvm_objcopy_path);
    llvm_objcopy.current_dir(&bootloader_target_path);
    llvm_objcopy.args(["-O", "binary", "bootloader", "bootloader.bin"]);
    assert!(llvm_objcopy.status().expect("Failed to execute llvm-objcopy on bootloader").success(),
        "Error while running llvm-objcopy on bootloader");

    // rebuild if any source file changed
    println!("cargo:rerun-if-changed={}", Path::new(&kernel_path).join("src").to_string_lossy());
    println!("cargo:rerun-if-changed={}", Path::new(&kernel_path).join("Cargo.toml").to_string_lossy());
    println!("cargo:rerun-if-changed={}", Path::new(&bootloader_path).join("src").to_string_lossy());
    println!("cargo:rerun-if-changed={}", Path::new(&bootloader_path).join("Cargo.toml").to_string_lossy());
}
