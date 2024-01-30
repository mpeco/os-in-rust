A simple x86_64 OS I wrote with the purpose of learning OS development and Rust.

Since the purpose of the project was to learn OS development I decided against using any external crate so everything would be implemented from scratch, therefore the only dependency is Rust's core library (the one exception being [paste](https://crates.io/crates/paste) for macros).

As of now the project consists of a simple bootloader and kernel with the following features:

* Paging;
* Heap allocation;
* Hardware interrupts (using the APIC, no PIC implementation);
* SMP support;
* A timer service (using the APIC timer or TSC if supported by the CPU);
* A preemptive scheduler.

It's hard to call it an OS in its current state, considering there's not much to do with it besides typing on a blank screen. Although I believe most of the foundation is set up for implementing a shell and some applications.

## Building

Compiling the OS is done with a build script that links the bootloader and kernel together into a single binary. The rustup component llvm-tools is required for the build script to run, it can be installed with the following command:

~~~~
rustup component add llvm-tools
~~~~

The building can be done as per usual with Rust (-vv will show the output of compiling the kernel and bootloader which are each contained in a different cargo packages, --release will only apply to the kernel):

~~~~
cargo build -vv
~~~~

## Running

A program for running the OS in QEMU is contained in the main cargo package of the repository, therefore the usual rust command:

~~~~
cargo run
~~~~

Should run it in QEMU, for this "qemu-img" and "qemu-system-x86_64" are required.

A few flags for altering QEMU options are also available:

* `"m [AMOUNT OF MEMORY IN MEGABYTES]"`
* `smp [NUMBER OF PROCESSORS]`
* `kvm`
* `invtsc`

"invtsc" is required for the timer to function using TSC and it requires "kvm", in case the user doesn't have the permissions for KVM you'd have to run the runner program directly with something like:

~~~~
sudo target/[debug/release]/os kvm invtsc
~~~~

## Used Resources
[OSDev Wiki](https://wiki.osdev.org/Main_Page)

[Writing an OS in Rust](https://os.phil-opp.com/)