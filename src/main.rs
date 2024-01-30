use std::{env, path::Path, process::Command};


fn main() {
    let args: Vec<String> = env::args().collect();
    let binary_path = Path::new("./bootloader/target/x86-bootloader-target/release/bootloader.bin");

    // create disk img
    let mut qemu_img = Command::new("qemu-img");
    let if_path_arg = format!("if={}", binary_path.to_string_lossy());
    qemu_img.args(["dd", "-f", "raw", "-O", "raw", &if_path_arg, "of=disk.img", "bs=512"]);
    assert!(qemu_img.status().unwrap().success(), "Failed to create disk img");

    // resize disk img (FIXME: would need to be updated if binary is bigger than 1M)
    let mut qemu_img = Command::new("qemu-img");
    qemu_img.args(["resize", "disk.img", "1M"]);
    assert!(qemu_img.status().unwrap().success(), "Failed to resize disk img");

    // setup qemu command
    let mut qemu = Command::new("qemu-system-x86_64");
    qemu.args(["-hda", "disk.img", "-monitor", "stdio"]);
    let mut machine_args = vec!["-machine", "q35"];

    let mut was_kvm_found = false;
    for (i, arg) in args.iter().enumerate().skip(1) {
        match arg.to_lowercase().as_str() {
            "m" => {
                if args.len() > i+1 {
                    if let Ok(val) = args[i+1].parse::<usize>() {
                        let val_as_str = format!("{}M", val);
                        qemu.args(["-m", &val_as_str]);
                    }
                    else {
                        panic!("m arg followed by invalid memory size {}", args[i+1]);
                    }
                }
                else {
                    panic!("m arg not followed by a memory size");
                }
            }
            "smp" => {
                if args.len() > i+1 {
                    if let Ok(val) = args[i+1].parse::<u16>() {
                        let val_as_str = val.to_string();
                        qemu.args(["-smp", &val_as_str]);
                    }
                    else {
                        panic!("smp arg followed by invalid processor number {}", args[i+1]);
                    }
                }
                else {
                    panic!("m arg not followed by number of processors");
                }
            }
            "kvm" => {
                machine_args.pop();
                machine_args.push("q35,accel=kvm");
                qemu.arg("-enable-kvm");
                was_kvm_found = true;
            }
            "invtsc" => {
                if was_kvm_found == false {
                    panic!("invtsc arg found before kvm arg");
                }
                qemu.args(["-cpu", "host,+invtsc"]);
            }
            _ => { continue; }
        }
    }

    qemu.args(machine_args);

    // run with qemu
    assert!(qemu.status().unwrap().success(), "Failed to run QEMU");
}
