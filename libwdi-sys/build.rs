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

fn _print_env(environment: &[(OsString, OsString)])
{
    eprintln!("Environment:");
    for pair in environment {
        let name = pair.0.to_string_lossy();
        let val = pair.1.to_string_lossy();
        if val.contains(";") {
            let vals = val.split(";");
            eprintln!("\t{}", name);
            for val in vals {
                eprintln!("\t\t{}", val);
            }
        } else {
            eprintln!("\t{}\t{}", pair.0.to_string_lossy(), pair.1.to_string_lossy());
        }
    }
}

//fn manually_compile()
//{
    //let mut cmd = Command::new("C:/Program Files (x86)/Microsoft Visual Studio/2019/Community/VC/Tools/MSVC/14.29.30133/bin/Hostx64/x64/cl.exe");
    //cmd
        //.env(
            //"LIB",
            //&[
                //"C:/Program Files (x86)/Microsoft Visual Studio/2019/Community/VC/Tools/MSVC/14.29.30133/lib/x64",
                //"C:/Program Files (x86)/Windows Kits/10/Lib/10.0.19041.0/ucrt/x64",
                //"C:/Program Files (x86)/Windows Kits/10/Lib/10.0.19041.0/um/x64",
            //].join(";")
        //)
        //.env(
            //"INCLUDE",
            //&[
                //"C:/Program Files (x86)/Microsoft Visual Studio/2019/Community/VC/Tools/MSVC/14.29.30133/include",
                //"C:/Program Files (x86)/Windows Kits/10/Include/10.0.19041.0/ucrt",
                //"C:/Program Files (x86)/Windows Kits/10/Include/10.0.19041.0/um",
                //"C:/Program Files (x86)/Windows Kits/10/Include/10.0.19041.0/cppwinrt",
                //"C:/Program Files (x86)/Windows Kits/10/Include/10.0.19041.0/winrt",
                //"C:/Program Files (x86)/Windows Kits/10/Include/10.0.19041.0/shared",
            //].join(";")
        //)
        //.args(&[
            //"-nologo",
            //"-MT",
            //"-Z7",
            //"-Brepro",
            //"-I",
            //"libwdi\\msvc",
            //"-I",
            //"\"libwdi\\libwdi\"",
            //"-W4",
            //"-c",
            //"libwdi\\libwdi\\embedder.c",
        //])
    //;

    //eprintln!("Command: {:#?}", cmd);
    ////_print_env(
        ////cmd
            ////.get_envs()
            ////.map(|(k, v)| (k.to_os_string(), v.unwrap().to_os_string()))
            ////.collect::<Vec<_>>()
            ////.as_slice()
    ////);
    //_print_env(
        //env::vars_os()
            //.collect::<Vec<_>>()
            //.as_slice()
    //);
    //cmd.status().unwrap().exit_ok().unwrap();
//}

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
    //let installer_path_patch = std::fs::read_to_string("installer_path.patch").unwrap();
    //let installer_path_patch = Patch::from_str(&installer_path_patch).unwrap();
    //// Strip the leading `a/` from the filename.
    //let filename = &installer_path_patch.original().unwrap()[2..];
    //eprintln!("{:?}", filename);
}


fn make_installer<P: AsRef<Path>>(out_dir: P, src_dir: P, include_dir: P)
{
    let out_dir = out_dir.as_ref();
    let src_dir = src_dir.as_ref();
    let include_dir = include_dir.as_ref();

    match std::fs::create_dir(out_dir.join("libwdi")) {
        Ok(_) => Ok(()),
        Err(e) => match e.kind() {
            std::io::ErrorKind::AlreadyExists => Ok(()),
            _ => Err(e),
        },
    }.unwrap();

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
        //.define("SOLUTIONDIR", Some(r#""..\..\..\""#))
        //.define("SOLUTIONDIR", "\"..\\\\..\\\\..\\\\\"")
        .flag_if_supported("/Oi") // Enable intrinsic functions.
        .flag_if_supported("/MT") // Runtime library: Multi-threaded.
        .flag_if_supported("/Zc:wchar_t") // Treat wchar_t as built-in type.
        .flag_if_supported("/TC") // Compile as C code
        //.out_dir(&out_dir)
        .file(src_dir.join("embedder.c"));

    let output_path = src_dir.join("embedder.exe");
    dbg!(&output_path);
    let mut command = embedder.get_compiler().to_command();
    command
        .arg("libwdi/libwdi/embedder.c")
        //.arg("/Felibwdi/libwdi/embedder.exe")
        //.arg(format!("/Fe{}", out_dir.join("embedder.exe").display()))
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
    //let embedder_path: &Path = embedder_path.as_ref();
    //let mut cmd = Command::new("C:/Users/Mikaela/code/1b2/bmputil/libwdi-sys/libwdi/libwdi/embedder.exe");
    dbg!(out_dir);
    let mut cmd = Command::new(src_dir.join("embedder.exe"));
    cmd
        .current_dir(src_dir)
        .arg("embedded.h")
        .status().unwrap().exit_ok().unwrap();
    
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

    //std::fs::copy(src_dir.join("winusb.inf.in"), out_dir.join("libwdi"))
        //.ignore_already_exists()
        //.unwrap();

    run_embedder(&out_dir, &src_dir);

    //println!("cargo:include={}", include_dir.to_str().unwrap());

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
        //.file(src_dir.join("libwdi.c"))
        //.file(src_dir.join("libwdi_dlg.c"))
        //.file(src_dir.join("logging.c"))
        //.file(src_dir.join("pki.c"))
        //.file(src_dir.join("tokenizer.c"))
        //.file(src_dir.join("vid_data.c"))
        .compile("wdi")
    ;

    // libwdi system library dependencies.
    println!("cargo:rustc-link-lib=Shell32");
    println!("cargo:rustc-link-lib=Ole32");

}
