#![feature(exit_status_error)]

use std::env;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::process::Command;
use std::path::{Path, PathBuf};

use log::{LevelFilter, info, error};
use diffy::Patch;


/// Turns a [cc::Build] into a [Command] that can be used to create an executable,
/// since cc-rs doesn't directly support compiling executables.
trait CcOutputExecutable
{
    fn output_executable(&self) -> Command;
}
impl CcOutputExecutable for cc::Build
{
    fn output_executable(&self) -> Command
    {
        self.get_compiler().to_command()
    }
}

pub struct LibwdiBuild
{
    /// Absolute path to the current working directory (should be libwdi-sys crate).
    cwd: PathBuf,

    /// The OUT_DIR environment variable Cargo sets for us.
    out_dir: PathBuf,

    /// The path to the base libwdi repo which is a submodule of this Git repo.
    libwdi_repo: PathBuf,

    /// The base directory we'll use for libwdi sources.
    /// This will be a subdirectory of out_dir.
    libwdi_src: PathBuf,
}

impl LibwdiBuild
{
    pub fn new() -> Self
    {
        let cwd = env::current_dir().unwrap();
        let out_dir = PathBuf::from(env::var("OUT_DIR").expect("Cargo always sets OUT_DIR"));
        let libwdi_repo = PathBuf::from(std::env::current_dir().unwrap()).join("libwdi");
        let libwdi_src = out_dir.join("libwdi");

        Self {
            cwd,
            out_dir,
            libwdi_repo,
            libwdi_src,
        }
    }

    pub fn apply_common_cc_options(&self, build: &mut cc::Build)
    {
        build
            .include(self.libwdi_src.join("libwdi"))
            .include(self.libwdi_src.join("msvc"))
            .define("_CRT_SECURE_NO_WARNINGS", None)
            .define("_WINDLL", None) // FIXME: Do we want this one?
            .define("_UNICODE", None)
            .define("UNICODE", None)
            .define("WIN64", None)
            .flag_if_supported("/Oi") // Enable intrinsic functions
            .flag_if_supported("/MT") // Runtime library: multi-threaded.
            .flag_if_supported("/Zc:wchar_t") // Treat wchar_t as a built-in type.
            .flag_if_supported("/TC") // Compile as C code.
        ;

    }

    /// Copies and patches libwdi source files to a source hierarchy in OUT_DIR/libwdi.
    pub fn populate_source_tree(&self)
    {
        // The source files that we'll copy, but need to be patched first, and their patch files.
        // Source files relative from libwdi_repo; patch files relative from current directory.
        let needs_patch = &[
            (Path::new("libwdi/libwdi.c"), Path::new("static_windows_error_str.patch")),
            (Path::new("libwdi/embedder.h"), Path::new("installer_path.patch")),
            (Path::new("msvc/config.h"), Path::new("winusb_only.patch")),
        ];

        // The source files that we'll copy as-is.
        // Again relative from libwdi_repo (after that .map() below, at least).
        let mut as_is: Vec<PathBuf> = [
            "libwdi_dlg.c",
            "logging.c",
            "pki.c",
            "tokenizer.c",
            "vid_data.c",
            "embedder.c",
            "installer.c",
            "libwdi.h",
            "libwdi_i.h",
            "tokenizer.h",
            "logging.h",
            "stdfn.h",
            "resource.h",
            "mssign32.h",
            "installer.h",
            "embedder_files.h",
            "msapi_utf8.h",
            "winusb.inf.in",
            "libusb0.inf.in",
            "libusbk.inf.in",
            "usbser.inf.in",
            "winusb.cat.in",
            "libusb0.cat.in",
            "libusbk.cat.in",
            "usbser.cat.in",
        ]
            .into_iter()
            .map(|filename| Path::new("libwdi").join(filename))
            .collect();

        // If we're NOT cross compiling, we also need to copy the other headers in the "msvc" tree.
        if cfg!(windows) {
            let msvc_headers = [
                "inttypes.h",
                "stdint.h",
            ]
                .into_iter()
                .map(|filename| Path::new("msvc").join(filename));
            as_is.extend(msvc_headers);
        }


        // Let Cargo know we depend on aaaaaall of these files.
        let mut all_needed_files: Vec<PathBuf> = Vec::with_capacity(as_is.len() + needs_patch.len());
        all_needed_files.extend(needs_patch.map(|item| self.libwdi_repo.join(item.0)));
        all_needed_files.extend(needs_patch.map(|item| self.cwd.join(item.1)));
        all_needed_files.extend(as_is.iter().map(|path| self.libwdi_repo.join(path)));

        for file in all_needed_files {
            println!("cargo:rerun-if-changed={}", file.display());
        }


        // Create the directories we need inside OUT_DIR to copy the source tree.

        let needed_directories = [
            self.libwdi_src.join("libwdi"),
            self.libwdi_src.join("msvc"),
        ];

        for dir in needed_directories {
            fs::create_dir_all(&dir)
                .expect(&format!("Error creating {} directory", dir.display()));
        }


        // Almost done. Patch the sources we need to patch...
        for (src_file, patch_file) in needs_patch {
            self.apply_patch_file(src_file, patch_file);
        }

        // Copy the rest of the sources over...
        for src_file in as_is {
            let repo_file = self.libwdi_repo.join(&src_file);
            let target_file = self.libwdi_src.join(&src_file);
            fs::copy(&repo_file, &target_file)
                .expect(&format!("Error copying {} to {}", repo_file.display(), target_file.display()));
        }


        // And finally, create one last header file from scratch: build64.h.
        // libwdi needs it with this simple define.

        let build64_path = self.libwdi_src.join("libwdi/build64.h");
        let mut build64 = File::create(&build64_path)
            .expect(&format!("Error creating {}", build64_path.display()));
        writeln!(build64, "#define BUILD64\n")
            .expect(&format!("Error writing to {}", build64_path.display()));

        // Flushing is apparently not enough, on Windows.
        drop(build64_path);


        // And we're done! :tada:
    }

    /// `src_file` should be a relative path from source tree base, e.g. `libwdi/libwdi.c`.
    /// `patch_file` should be either absolute or relative from the current working directory.
    fn apply_patch_file<SrcP, PatchP>(&self, src_file: SrcP, patch_file: PatchP)
    where
        SrcP: AsRef<Path>,
        PatchP: AsRef<Path>
    {
        let src_file = src_file.as_ref();
        let patch_file = patch_file.as_ref();

        let base_path = self.libwdi_repo.join(src_file);
        let target_path = self.libwdi_src.join(src_file);

        info!("Applying {} to {}", patch_file.display(), target_path.display());

        // First, create the Patch structure from the patch file.
        let patch_text = fs::read_to_string(patch_file)
            .expect(&format!("Error reading patch file {}", patch_file.display()));
        let patch = Patch::from_str(&patch_text)
            .expect(&format!("Patch file {} seems invalid", patch_file.display()));

        // Now read the file we need to patch, and then patch its text.
        let base_text = fs::read_to_string(&base_path)
            .expect(&format!("Error reading source file {} for patching", src_file.display()));
        let patched_text = diffy::apply(&base_text, &patch)
            .expect(&format!("Error applying patch {} to source file {}", patch_file.display(), src_file.display()));

        // Finally, write the patched text to the target source tree (in OUT_DIR).
        fs::write(&target_path, &patched_text)
            .expect(&format!("Error writing patched source file {}", target_path.display()));
    }

    pub fn make_embedder(&self)
    {
        info!("Building embedder host binary...");

        let mut embedder = cc::Build::new();
        embedder
            .static_crt(true)
            .target(&env::var("HOST").expect("Cargo always sets HOST"))
            .include(self.libwdi_src.join("libwdi"))
            .include(self.libwdi_src.join("msvc"))
        ;

        // Allow the user to specify WDK_DIR environment variable to override the default WDK
        // directory. This becomes necessary when cross compiling.
        if let Ok(val) = env::var("WDK_DIR") {
            embedder.define("WDK_DIR", Some(format!(r#""{}""#, val).as_str()));
        }
        println!("cargo:rerun-if-env-changed=WDK_DIR");

        // If we're cross compiling...
        if !cfg!(windows) {
            // config.h errors if _MSC_VER isn't defined, so let's just define it.
            embedder.define("_MSC_VER", "1929");

            // Also let the user know that WDK_DIR is required when cross compiling.
            if let Err(_e) = env::var("WDK_DIR") {
                error!("WDK_DIR environment variable required when cross compiling");
                error!("Hint: Download the WDK 8.0 redistributable components here: https://learn.microsoft.com/en-us/windows-hardware/drivers/other-wdk-downloads\nand then set $WDK_DIR to something like '/opt/Program Files/Windows Kits/8.0' depending on where you extracted the WDK");
                panic!("WDK_DIR environment variable required when cross compiling");
            }
        }

        // Technically this is a host binary, but it's not like a host binary ending in .exe is an
        // error on Linux or macOS.
        let output_path = self.out_dir.join("embedder.exe");
        self.apply_common_cc_options(&mut embedder);
        embedder
            .flag_if_supported(&format!("/Fe{}", output_path.display())) // MSVC-style.
            .flag_if_supported(&format!("-o{}", output_path.display())); // GCC-style.


        // Turn this cc::Build into a command that we can execute ourselves.

        let mut cc_cmd = embedder.output_executable();

        // Add the C source file, because cc::Build::file() doesn't survive the conversion to a Command.
        cc_cmd.arg(self.libwdi_src.join("libwdi/embedder.c").as_os_str());

        info!("{:?}", cc_cmd);

        cc_cmd
            .status()
            .unwrap()
            .exit_ok()
            .expect("Compiler returned non-zero exit code");
    }

    pub fn make_installer(&self)
    {
        // The equivalent msbuild command to build this manually:
        // $MSBUILD libwdi/.msvc/installer_x64.vcxproj -p:PlatformToolset=142
        // -p:PerferredToolArchitecture=x64 -p:Platform=x64 -p:Configuration=Release

        // Which runs roughly this command:
        // cl.exe /c /I..\..\msvc /Zi /nologo /W3 /WX- /O1 /GL /D _CRT_SECURE_NO_WARNINGS /D _WIN64
        // /D _WINDLL /D _UNICODE /Gm- /EHsc /MT /GS /fp:precise /Qspectre /Zc:wchar_t /Zc:forScope
        // /Zc:inline /external:W3 /Gd /TC /FC ..\installer.c


        info!("Building installer_x64...");

        let output_path = self.out_dir.join("installer_x64.exe");

        let mut installer = cc::Build::new();
        self.apply_common_cc_options(&mut installer);

        let mut cc_cmd = installer
            .static_crt(true)
            .flag(&format!("/Fe{}", output_path.display()))
            .output_executable();

        cc_cmd
            // Add the C source file, because cc::Build::file() doesn't survive the conversion
            // to a Command.
            .arg(self.libwdi_src.join("libwdi/installer.c"))
            // Add the WinAPI libraries we need to link against.
            .args(&[
                "/link",
                "newdev.lib",
                "setupapi.lib",
                "user32.lib",
                "ole32.lib",
                "advapi32.lib",
            ]);

        info!("{:?}", cc_cmd);

        cc_cmd
            .status()
            .unwrap()
            .exit_ok()
            .expect("Compiler returned non-zero exit code");
    }

    pub fn run_embedder(&self)
    {
        info!("Running embedder host binary...");

        let mut cmd = Command::new(self.out_dir.join("embedder.exe"));
        cmd
            .current_dir(&self.libwdi_src.join("libwdi"))
            .arg("embedded.h");
        info!("{:?}", cmd);

        cmd
            .status()
            .unwrap()
            .exit_ok()
            .expect("Embedder executable returned non-zero exit code");
    }

    pub fn make_lib(&self)
    {
        info!("Building libwdi static library...");

        let lib_srcs: Vec<PathBuf> = [
            "libwdi.c",
            "libwdi_dlg.c",
            "logging.c",
            "pki.c",
            "tokenizer.c",
            "vid_data.c",
        ]
            .into_iter()
            .map(|path| self.libwdi_src.join("libwdi").join(path))
            .collect();

        let mut lib = cc::Build::new();
        self.apply_common_cc_options(&mut lib);
        lib
            .files(&lib_srcs)
            .compile("wdi");

        println!("cargo:include={}", self.libwdi_src.join("libwdi").to_str().unwrap());
        println!("cargo:rustc-link-lib=shell32");
        println!("cargo:rustc-link-lib=ole32");
    }

    pub fn run_bindgen(&self)
    {
        // HACK: attempt to find libclang.dll from Visual Studio.
        let msvc = cc::windows_registry::find_tool("x86_64-pc-windows-msvc", "vcruntime140.dll")
            .expect("Failed to find MSVC");
        let msvc_path = msvc.path();

        let clang_dir = msvc_path // cl.exe
            .parent().unwrap() // x64
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
            //.clang_arg("-Ilibwdi/libwdi")
            .allowlist_function("wdi_.*")
            .allowlist_var("wdi_.*")
            .allowlist_type("wdi_.*")
            .prepend_enum_name(false)
            .detect_include_paths(true)
            .generate()
            .expect("Unable to generate bindings");

        bindings
            .write_to_file(self.out_dir.join("bindings.rs"))
            .expect("Couldn't write bindings");
    }
}


fn main()
{
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info)
        .parse_default_env()
        .init();

    let build = LibwdiBuild::new();
    build.populate_source_tree();
    build.make_embedder();
    build.make_installer();
    build.run_embedder();
    build.make_lib();

    if cfg!(feature = "dynamic-bindgen") {
        build.run_bindgen();
    }
}
