use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use gl_generator::{Api, Fallbacks, Profile, Registry, StructGenerator};

fn main() {
    let out = env::var("OUT_DIR").unwrap();
    let out = Path::new(&out);

    {
        let mut file = File::create(out.join("gl_bindings.rs")).unwrap();

        Registry::new(
            Api::Gles2,
            (3, 0),
            Profile::Core,
            Fallbacks::All,
            [
                "GL_EXT_memory_object",
                "GL_EXT_memory_object_fd",
                "GL_EXT_memory_object_win32",
                "GL_EXT_semaphore",
                "GL_EXT_semaphore_fd",
                "GL_EXT_semaphore_win32",
            ],
        )
        .write_bindings(StructGenerator, &mut file)
        .unwrap();
    }

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    if target_os == "macos" {
        println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/lib/");
    }

    if target_os == "android" {
        let mut libgcc = File::create(out.join("libgcc.a")).unwrap();
        libgcc.write_all(b"INPUT(-lunwind)").unwrap();
        println!("cargo:rustc-link-search=native={}", out.display());
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/discowl.slint");

    slint_build::compile("src/discowl.slint").unwrap();
}
