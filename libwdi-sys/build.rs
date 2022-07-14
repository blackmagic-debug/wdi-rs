#![feature(exit_status_error)]

use std::env;
use std::fs;
use std::fs::File;
use std::io::{Write, BufRead, BufReader};
use std::ffi::OsString;
use std::process::Command;
use std::path::{Path, PathBuf};

use diffy::Patch;

trait IoIgnoreAlreadyExists<E>
{
    fn ignore_already_exists(self) -> Result<(), E>;
}

impl<T> IoIgnoreAlreadyExists<std::io::Error> for std::io::Result<T>
{
    fn ignore_already_exists(self) -> Result<(), std::io::Error>
    {
        match self {
            Ok(v) => Ok(()),
            Err(e) => match e.kind() {
                std::io::ErrorKind::AlreadyExists => Ok(()),
                _ => Err(e)
            }
        }
    }
}

fn apply_patch_file<P: AsRef<Path>>(patch_file_path: P)
{
    let patch_file_path: &Path = patch_file_path.as_ref();
    let patch_text = fs::read_to_string(patch_file_path).unwrap();
    let patch = Patch::from_str(&patch_text).unwrap();

    // Strip the leading `a/` from the filename, and prepend the path to libwdi's repo.
    let filename = format!("libwdi/{}", &patch.original().unwrap()[2..]);

    // Get the file to patch, and patch its text.
    let base_text = fs::read_to_string(&filename).unwrap();
    let patched_text = match diffy::apply(&base_text, &patch) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Warning: patching {} failed, but this could just be because the patch has already been applied.", &filename);
            eprintln!("Warning: if building fails, this error might be the reason why: {:?}", e);
            return;
        },
    };

    // Write the patched text to the file.
    fs::write(&filename, &patched_text).unwrap();
}


fn patch_source<P: AsRef<Path>>(src_dir: P)
{
    apply_patch_file("installer_path.patch");
    apply_patch_file("winusb_only.patch");
}


fn make_installer<P: AsRef<Path>>(out_dir: P, src_dir: P, include_dir: P)
{
    let out_dir = out_dir.as_ref();
    let src_dir = src_dir.as_ref();
    let include_dir = include_dir.as_ref();

    std::fs::create_dir(out_dir.join("libwdi"))
        .ignore_already_exists()
        .unwrap();
    //match std::fs::create_dir(out_dir.join("libwdi")) {
        //Ok(_) => Ok(()),
        //Err(e) => match e.kind() {
            //std::io::ErrorKind::AlreadyExists => Ok(()),
            //_ => Err(e),
        //},
    //}.unwrap();

    let mut installer = cc::Build::new();

    installer
        .static_crt(true)
        .include(&include_dir)
        .include(&src_dir)
        .define("_CRT_SECURE_NO_WARNINGS", None)
        .define("_WINDLL", None)
        .define("_UNICODE", None)
        .define("UNICODE", None)
        .define("_WIN64", None)
        .flag_if_supported("/Oi") // Enable intrinsic functions.
        .flag_if_supported("/MT") // Runtime library: Multi-threaded.
        .flag_if_supported("/Zc:wchar_t") // Treat wchar_t as built-in type.
        .flag_if_supported("/TC") // Compile as C code
        .file(src_dir.join("installer.c"));

    let mut command = installer.get_compiler().to_command();
    command
        .arg(src_dir.join("installer.c"))
        //.arg(format!("/Fe{}", out_dir.join("libwdi").join("installer_x64.exe").display())) // Set output name.
        .arg(format!("/Fe{}", src_dir.join("installer_x64.exe").display())) // Set output name.
        .args(&[
            "/link",
            "newdev.lib",
            "setupapi.lib",
            "user32.lib",
            "ole32.lib",
            "Advapi32.lib",
        ])
        .status()
        .unwrap()
        .exit_ok()
        .unwrap();


    // $MSBUILD libwdi/.msvc/installer_x64.vcxproj -p:PlatformToolset=v142
    // -p:PreferredToolArchitecture=x64 -p:Platform=x64 -p:Configuration=Release

    // cl.exe /c /I..\..\msvc /Zi /nologo /W3 /WX- /O1 /GL /D _CRT_SECURE_NO_WARNINGS /D _WIN64 /D
    // _WINDLL /D _UNICODE /Gm- /EHsc /MT /GS /fp:precise /Qspectre /Zc:wchar_t /Zc:forScope
    // /Zc:inline /external:W3 /Gd /TC /FC ..\installer.c



    // $MSBUILD libwdi/.msvc/embedder.vcxproj -p:PlatformToolset=v142
    // -p:PreferredToolArchitecture=x64 -p:Configuration=Release
}

fn make_embedder<P: AsRef<Path>>(out_dir: P, src_dir: P, include_dir: P)
{
    eprintln!("Building embedder host binary...");
    let out_dir = out_dir.as_ref();
    let src_dir = src_dir.as_ref();
    let include_dir = include_dir.as_ref();

    let mut embedder = cc::Build::new();
    embedder
        .static_crt(true)
        .include(&include_dir)
        .include(&src_dir)
        //.include("C:/Users/Mikaela/code/1b2/bmputil/libwdi-sys/libwdi")
        .define("_CRT_SECURE_NO_WARNINGS", None)
        .define("_WINDLL", None)
        .define("_UNICODE", None)
        .define("UNICODE", None)
        .flag_if_supported("/Oi") // Enable intrinsic functions.
        .flag_if_supported("/MT") // Runtime library: Multi-threaded.
        .flag_if_supported("/Zc:wchar_t") // Treat wchar_t as built-in type.
        .flag_if_supported("/TC") // Compile as C code
        .file(src_dir.join("embedder.c"));

    let output_path = src_dir.join("embedder.exe");
    let mut command = embedder.get_compiler().to_command();
    command
        .arg("libwdi/libwdi/embedder.c")
        .arg(format!("/Fe{}", output_path.display()))
        .status()
        .unwrap()
        .exit_ok()
        .unwrap();
}

fn run_embedder<POutDirT, PSrcDirT>(out_dir: POutDirT, src_dir: PSrcDirT)
where
    POutDirT: AsRef<Path>,
    PSrcDirT: AsRef<Path>,
{
    let out_dir: &Path = out_dir.as_ref();
    let src_dir: &Path = src_dir.as_ref();
    let mut cmd = Command::new(src_dir.join("embedder.exe"));
    cmd
        .current_dir(src_dir)
        .arg("embedded.h")
        .status().unwrap().exit_ok().unwrap();

}


fn run_bindgen<P: AsRef<Path>>(out_dir: P)
{
    let out_dir: &Path = out_dir.as_ref();

    // HACK: attempt to find libclang.dll from Visual Studio.
    let msvc = cc::windows_registry::find_tool("x86_64-pc-windows-msvc", "vcruntime140.dll")
        .expect("Failed to find MSVC");
    let msvc_path = msvc.path();

    let clang_dir = msvc_path // cl.exe
        .parent().unwrap() //  x64
        .parent().unwrap() // HostX64
        .parent().unwrap() // bin
        .parent().unwrap() // <VC Tools version>
        .parent().unwrap() // MSVC
        .parent().unwrap() // Tools
        .join(&["Llvm", "x64", "bin"].into_iter().collect::<PathBuf>());
    dbg!(clang_dir.join("libclang.dll"));
    env::set_var("LIBCLANG_PATH", clang_dir.join("libclang.dll"));

    println!("cargo:rerun-if-changed=wrapper.h");

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .clang_arg("-Ilibwdi/libwdi")
        .allowlist_function("wdi_.*")
        .allowlist_var("wdi_.*")
        .allowlist_type("wdi_.*")
        .prepend_enum_name(false)
        .detect_include_paths(true)
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Couldn't write bindings");
}


fn main()
{
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let libwdi_repo = PathBuf::from(std::env::current_dir().unwrap()).join("libwdi");
    let include_dir = PathBuf::from(&libwdi_repo).join("msvc");
    let src_dir = PathBuf::from(&libwdi_repo).join("libwdi");

    let mut build64 = File::create(src_dir.join("build64.h")).unwrap();
    writeln!(build64, "#define BUILD64\n").unwrap();
    drop(build64); // Apparently flushing is not enough, on Windows.

    patch_source(&src_dir);

    make_embedder(&out_dir, &src_dir, &include_dir);
    make_installer(&out_dir, &src_dir, &include_dir);

    std::thread::sleep(std::time::Duration::from_secs(1)); // XXX

    run_embedder(&out_dir, &src_dir);

    let libwdi_srcs = [
        "libwdi.c",
        "libwdi_dlg.c",
        "logging.c",
        "pki.c",
        "tokenizer.c",
        "vid_data.c",
    ].map(|src_path| src_dir.join(src_path));



    let mut build = cc::Build::new();
    build
        .include(&include_dir)
        .include(&libwdi_repo)
        .files(&libwdi_srcs)
        .compile("wdi");

    if cfg!(feature = "dynamic-bindgen") {
        run_bindgen(&out_dir);
    }

    // libwdi system library dependencies.
    println!("cargo:rustc-link-lib=Shell32");
    println!("cargo:rustc-link-lib=Ole32");
}
