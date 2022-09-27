#![feature(exit_status_error)]

use std::env;
use std::fs;
use std::fs::File;
use std::io::Write;
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

fn apply_patch_file<SrcP, PatchP>(src_dir: SrcP, patch_file_path: PatchP)
where
    SrcP: AsRef<Path>,
    PatchP: AsRef<Path>,
{
    let src_dir = src_dir.as_ref();
    let patch_file_path: &Path = patch_file_path.as_ref();
    let patch_text = fs::read_to_string(patch_file_path).unwrap();
    let patch = Patch::from_str(&patch_text).unwrap();

    // Strip the leading `a/` from the filename, and prepend the path to libwdi's repo.
    // src_dir already includes the `libwdi` part, which is also included in the patch files.
    // So we'll start from src_dir's parent.
    let filename = src_dir.parent().unwrap().join(&patch.original().unwrap()[2..]);

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
    let src_dir = src_dir.as_ref();
    apply_patch_file(src_dir, "installer_path.patch");
    apply_patch_file(src_dir, "winusb_only.patch");
    apply_patch_file(src_dir, "static_windows_error_str.patch");
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
        .arg("-fuse-ld=lld-link")
        .args(&[
            "/link",
            "newdev.lib",
            "setupapi.lib",
            "user32.lib",
            "ole32.lib",
            "AdvAPI32.Lib",
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
    let output_path = out_dir.join("embedder.exe");

    let mut embedder = cc::Build::new();
    embedder
        .static_crt(true)
        .target(&env::var("HOST").expect("HOST environment variable not present"));
    if !cfg!(windows) {
        embedder.define("_MSC_VER", "1929");
        if let Ok(val) = env::var("WDK_DIR") {
            embedder.define("WDK_DIR", Some(format!(r#""{}""#, val).as_str()));
        } else {
            eprintln!("Error: WDK_DIR environment variable required when cross compiling");
            eprintln!("Hint: Download the WDK 8.0 redistributable components here: https://learn.microsoft.com/en-us/windows-hardware/drivers/other-wdk-downloads and then set $WDK_DIR to something like '/opt/Program Files/Windows Kits/8.0' depending on where you extracted the WDK");
            std::process::exit(2);
        }
    } else {
        embedder.include(include_dir);
    }
    embedder
        .include(&src_dir)
        .define("_CRT_SECURE_NO_WARNINGS", None)
        .define("_WINDLL", None)
        .define("_UNICODE", None)
        .define("UNICODE", None)
        .flag_if_supported("/Oi") // Enable intrinsic functions.
        .flag_if_supported("/MT") // Runtime library: Multi-threaded.
        .flag_if_supported("/Zc:wchar_t") // Treat wchar_t as built-in type.
        .flag_if_supported("/TC") // Compile as C code
        .flag_if_supported(&format!("/Fe{}", output_path.display()))
        .flag_if_supported(&format!("-o{}", output_path.display()))
        .flag_if_supported("-fuse-ld=lld-link")
        .file(src_dir.join("embedder.c"));

    let mut command = embedder.get_compiler().to_command();
    command
        .arg("libwdi/libwdi/embedder.c");
        //.arg("-fuse-ld=lld-link")
        //.arg("-v")
        //.arg(format!("/Fe{}", output_path.display()));
    dbg!(&command);
    command
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
    eprintln!("Running embedder");
    let out_dir: &Path = out_dir.as_ref();
    let src_dir: &Path = src_dir.as_ref();
    let mut cmd = Command::new(out_dir.join("embedder.exe"));
    cmd
        .current_dir(src_dir)
        .arg("embedded.h");
    dbg!(&cmd);
    cmd.status().unwrap().exit_ok().expect("embedder executable returned non-zero exit code");

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
    println!("cargo:rustc-link-lib=shell32");
    println!("cargo:rustc-link-lib=ole32");
}
