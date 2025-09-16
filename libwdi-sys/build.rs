// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2022-2023 1BitSquared <info@1bitsquared.com>
// SPDX-FileContributor: Written by Mikaela Szekely <mikaela.szekely@qyriad.me>
use std::env;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::ffi::{OsStr, OsString};
use std::process::Command;
use std::path::{Path, PathBuf};

use log::{LevelFilter, info, error};
use diffy::Patch;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum BuildType
{
    Host,
    Target,
}


/// HACK: Converts an AsRef<Path> to something suitable for passing arguments to a command.
/// This is for working around an edge case when cross compiling using clang-cl on macOS,
/// where absolute filepaths are interpreted as the /U command-line switch for undefining
/// macros, instead of a file name. You can use `--` to indicate all following arguments are
/// filenames, but then you can't pass any more switches like /link.
/// Hence: this trait. This trait prepends a single slash to absolute paths starting with /U,
/// i.e.: `/Users/foo/bar/foobar.c` -> `//Users/foo/bar/foobar.c`.
trait PathToArg
{
    fn to_arg(&self) -> OsString;
}
impl<T: AsRef<Path>> PathToArg for T
{
    /// Prepends an extra leading slash to paths starting with /U, to workaround an edge case
    /// with clang-cl on macOS. See the trait-level documentation for more.
    fn to_arg(&self) -> OsString
    {
        let path = self.as_ref();
        let path_str = path.as_os_str();

        // OsStr does not have a .starts_with().
        // https://github.com/rust-lang/rfcs/issues/900.
        let path_lossy = path_str.to_string_lossy();

        // Only modify paths that start with /U. We don't use any other criteria for modification,
        // because either it's a path that starts with /U and needs to be escaped, or it's intended
        // to be the switch for undefining a macro, in which case this argument shouldn't be in
        // a [Path] anyway.
        if !path_lossy.starts_with("/U") {
            return path_str.to_owned();
        }

        let mut arg_str = OsString::with_capacity(path_str.len() + 1);
        arg_str.push(OsStr::new("/"));
        arg_str.push(path_str);

        arg_str
    }
}


fn getenv(v: &str) -> Option<String>
{
    env::var(v).ok()
}

/// Reimplementation of private function [cc::Build::get_var].
fn get_cc_var(var_base: &str) -> Option<String>
{
    let host = env::var("HOST")
        .expect("Cargo always sets HOST variable");
    let target = env::var("TARGET")
        .expect("Cargo always sets TARGET variable");
    let kind = if host == target { "HOST" } else { "TARGET" };

    let target_u = target.replace("-", "_");
    let res = getenv(&format!("{}_{}", var_base, target))
        .or_else(|| getenv(&format!("{}_{}", var_base, target_u)))
        .or_else(|| getenv(&format!("{}_{}", kind, var_base)))
        .or_else(|| getenv(var_base));

    match res {
        Some(res) => Some(res),
        None => None,
    }
}


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

struct LibwdiBuild
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
    fn new() -> Self
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

    /// A hack to apply necessary compiler options if we're compiling with cargo-xwin.
    /// This returns an empty Vec if we're not running under cargo-xwin.
    fn get_compiler_options(&self, target_arch: &str) -> Vec<String>
    {
        // cc-rs isn't meant for cross-compiling binaries like this, and so
        // doesn't bother letting the compiler know where the Windows SDK and
        // MSCRT are when targeting a different architecture to the intended one.
        // Let's detect if we're compiling under cargo-xwin, and if so scrape the
        // locations we need from the variables it does set, then compile a new
        // set of compiler flags for the intended architecture.

        let mut compile_args: Vec<String> = Vec::new();

        let cflags = match get_cc_var("CFLAGS") {
            Some(v) => v,
            None => return compile_args,
        };

        if !cflags.contains("cargo-xwin") {
            return compile_args;
        }
        let cargo_arch = env::var("CARGO_CFG_TARGET_ARCH")
            .expect("Cargo always sets CARGO_CFG_TARGET_ARCH");

        if cargo_arch == target_arch {
            return compile_args;
        }

        let args: Vec<&str> = cflags.split_whitespace().collect();
        for flag in args {
            if flag.starts_with("/imsvc") || flag.starts_with("-fuse-ld") {
                compile_args.push(String::from(flag))
            }
        }

        return compile_args;
    }

    /// A hack to apply necessary linker options if we're compiling with cargo-xwin.
    /// This returns an empty Vec if we're not running under cargo-xwin.
    fn get_linker_options(&self, target_arch: &str) -> Vec<String>
    {
        // cc-rs isn't meant for compiling binaries, and so cargo-xwin doesn't
        // bother letting the linker know where the Windows SDK and MSCRT are.
        // Let's detect if we're compiling under cargo-xwin, and if so scrape
        // the locations we need from the variables it *does* set, and then hack
        // in the linker flags.

        let mut link_args: Vec<String> = Vec::with_capacity(3);

        let cflags = match get_cc_var("CFLAGS") {
            Some(v) => v,
            None => return link_args,
        };

        if !cflags.contains("cargo-xwin") {
            return link_args;
        }
        let args: Vec<&str> = cflags.split_whitespace().collect();
        let crt_include_arg = args
            .iter()
            .find(|arg| arg.starts_with("/imsvc") && arg.contains("crt/include"));

        let crt_include_arg = match crt_include_arg {
            Some(v) => *v,
            None => return link_args,
        };

        // Should have the format `/imsvc{xwin_dir}/crt/include`.
        // So let's remove the `/imsvc` prefix and `/crt/include` suffix.
        let imsvc_len = "/imsvc".len();
        let without_imsvc = &crt_include_arg[imsvc_len..];
        let inc_idx = match without_imsvc.find("/crt/include") {
            Some(v) => v,
            None => return link_args,
        };

        let xwin_dir = Path::new(&without_imsvc[0..inc_idx]);

        let sdk_lib_dir = xwin_dir.join(&format!("sdk/lib/um/{}", &target_arch));
        let ucrt_lib_dir = xwin_dir.join(&format!("sdk/lib/ucrt/{}", &target_arch));
        let crt_lib_dir = xwin_dir.join(&format!("crt/lib/{}", &target_arch));

        for lib_dir in [sdk_lib_dir, ucrt_lib_dir, crt_lib_dir] {
            link_args.push(format!("/libpath:{}", lib_dir.to_str().unwrap()));
        }

        return link_args;
    }

    fn apply_common_cc_options(&self, build: &mut cc::Build, build_type: BuildType)
    {
        build
            .include(self.libwdi_src.join("libwdi"))
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

        if build_type == BuildType::Host && cfg!(not(windows)) {
            // If we're cross compiling generally, but this cc::Build is for the host,
            // add the special host-include path which includes config.h but not the msvc headers.
            build.include(self.libwdi_src.join("host-include"));
        } else {
            // Otherwise, make sure the "msvc" directory is in the include path.
            build.include(self.libwdi_src.join("msvc"));
        }

    }

    /// Copies and patches libwdi source files to a source hierarchy in OUT_DIR/libwdi.
    fn populate_source_tree(&self)
    {
        // The source files that we'll copy, but need to be patched first, and their patch files.
        // Source files relative from libwdi_repo; patch files relative from current directory.
        let needs_patch = &[
            // libwdi's embedder host program hardcodes the path to installer_x64.exe based on
            // Visual Studio's default directory structure (e.g. `x64/Release/helper`).
            // We're not using that, so let's patch that path.
            (Path::new("libwdi/embedder.h"), Path::new("installer_path.patch")),

            // libwdi doesn't let you simply not define driver file locations for libusb-win32
            // or libusbK to disable them, so let's cut default paths with a patch.
            // We still can enable them later by defining LIBUSB0_DIR or LIBUSBK_DIR
            (Path::new("msvc/config.h"), Path::new("no_default_paths.patch")),

            // libwdi's installer makes a mess of some types that makes ARM compilation
            // angry, so fix the type mistakes with a patch.
            (Path::new("libwdi/installer.c"), Path::new("installer_types_mismatches.patch")),
        ];

        // The source files that we'll copy as-is.
        // Again relative from libwdi_repo (after that .map() below, at least).
        let mut as_is: Vec<PathBuf> = [
            "libwdi.c",
            "libwdi_dlg.c",
            "logging.c",
            "pki.c",
            "tokenizer.c",
            "vid_data.c",
            "embedder.c",
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

        // Add the MSVC headers as well.
        let msvc_headers = [
            "inttypes.h",
            "stdint.h",
        ]
            .into_iter()
            .map(|filename| Path::new("msvc").join(filename));
        as_is.extend(msvc_headers);

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

        // Disable target support if flag is not set
        if cfg!(not(feature = "enable-x86")) {
            self.replace_in_file(Path::new("msvc/config.h"), "#define OPT_M32", "//#define OPT_M32");
        }
        if cfg!(not(feature = "enable-arm64")) {
            self.replace_in_file(Path::new("msvc/config.h"), "#define OPT_ARM", "//#define OPT_ARM");
        }

        // Minor hack: when cross compiling, the host needs a config.h, but needs to NOT have the
        // msvc headers in the include path. Let's create a special directory for that.
        if cfg!(not(windows)) {

            let host_include = self.libwdi_src.join("host-include");
            fs::create_dir_all(&host_include)
                .expect(&format!("Error creating {} directory", host_include.display()));

            // And copy config.h to it.
            fs::copy(self.libwdi_src.join("msvc/config.h"), self.libwdi_src.join("host-include/config.h"))
                .expect("Failed to copy config.h to host-include directory");
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

        // HACK: convert from crlf to lf, as diffy doesn't support Windows line endings.
        let patch_text: String = patch_text.chars().into_iter().filter(|c| *c != '\r').collect();
        let patch = Patch::from_str(&patch_text)
            .expect(&format!("Patch file {} seems invalid", patch_file.display()));

        // Now read the file we need to patch, and then patch its text.
        let base_text = fs::read_to_string(&base_path)
            .expect(&format!("Error reading source file {} for patching", src_file.display()));

        // HACK: convert from crlf to lf, as diffy doesn't support Windows line endings.
        let base_text: String = base_text.chars().into_iter().filter(|c| *c != '\r').collect();
        let patched_text = diffy::apply(&base_text, &patch)
            .expect(&format!("Error applying patch {} to source file {}", patch_file.display(), src_file.display()));

        // Finally, write the patched text to the target source tree (in OUT_DIR).
        fs::write(&target_path, &patched_text)
            .expect(&format!("Error writing patched source file {}", target_path.display()));
    }

    fn replace_in_file<TargetP>(&self, target_file: TargetP, target_str: &str, replacement: &str)
    where
        TargetP: AsRef<Path>
    {
        let target_path = self.libwdi_src.join(target_file);
        let content = fs::read_to_string(&target_path)
            .expect(&format!("Error reading source file for replace {}", target_path.display()));
        let new_content = content.replace(target_str, replacement);
        fs::write(&target_path, new_content)
            .expect(&format!("Error writing source file for replace {}", target_path.display()));
    }

    /// Compiles the embedder host binary that is needed to compile libwdi.
    ///
    /// libwdi's normal build process involves compiling an executable, which is then run
    /// during the build process to generate a C header file which contains the bytes of
    /// the output of [make_installer_x86_64]. This embedder binary thus must be a host executable.
    /// It also has to be built before the other build steps.
    /// [cc] does not support building executables, directly, so this function contains some hacks
    /// to try to get it to work.
    ///
    /// If something in this build script blows up, it's probably this, especially during cross
    /// compilation.
    fn make_embedder(&self)
    {
        info!("Building embedder host binary...");

        let mut embedder = cc::Build::new();
        embedder
            .static_crt(true)
            .target(&env::var("HOST").expect("Cargo always sets HOST"))
            .include(self.libwdi_src.join("libwdi"));

        // Allow the user to specify WDK_DIR environment variable to override the default WDK
        // directory. This becomes necessary when cross compiling.
        if let Ok(val) = env::var("WDK_DIR") {
            embedder.define("WDK_DIR", Some(format!(r#""{}""#, val).as_str()));
        }
        println!("cargo:rerun-if-env-changed=WDK_DIR");

        // If we're compiling with libusb0, let the embedder know where it is.
        if cfg!(feature = "libusb0") {
            if let Ok(val) = env::var("LIBUSB0_DIR") {
                embedder.define("LIBUSB0_DIR", Some(format!(r#""{}""#, val).as_str()));
            } else {
                error!("LIBUSB0_DIR environment variable required when compiling with libusb0");
                panic!("LIBUSB0_DIR environment variable required when compiling with libusb0");
            }
            println!("cargo:rerun-if-env-changed=LIBUSB0_DIR");
        }
        // Ditto for libusbk
        if cfg!(feature = "libusbk") {
            if let Ok(val) = env::var("LIBUSBK_DIR") {
                embedder.define("LIBUSBK_DIR", Some(format!(r#""{}""#, val).as_str()));
            } else {
                error!("LIBUSBK_DIR environment variable required when compiling with libusbk");
                panic!("LIBUSBK_DIR environment variable required when compiling with libusbk");
            }
            println!("cargo:rerun-if-env-changed=LIBUSBK_DIR");
        }

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
        } else {
            embedder.include(self.libwdi_src.join("msvc"));
        }

        // Technically this is a host binary, but it's not like a host binary ending in .exe is an
        // error on Linux or macOS.
        let output_path = self.out_dir.join("embedder.exe");
        self.apply_common_cc_options(&mut embedder, BuildType::Host);
        embedder
            .flag_if_supported(&format!("/Fe{}", output_path.display())) // MSVC-style.
            .flag_if_supported(&format!("-o{}", output_path.display())); // GCC-style.


        // Turn this cc::Build into a command that we can execute ourselves.

        let mut cc_cmd = embedder.output_executable();

        // Add the C source file, because cc::Build::file() doesn't survive the conversion to a Command.
        cc_cmd.arg(self.libwdi_src.join("libwdi/embedder.c").as_os_str());

        info!("{:?}", cc_cmd);

        let success = cc_cmd
            .current_dir(&self.out_dir)
            .status()
            .unwrap()
            .success();
        if !success {
            panic!("Compiler returned non-zero exit code");
        }
    }

    /// Builds the `installer_x64.exe` binary which gets embedded into a C header for the
    /// rest of the library.
    ///
    /// Like [make_embedder], this function also contains hacks to use [cc] to compile an
    /// executable instead of a library.
    fn make_installer_x86_64(&self)
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

        // HACK: if we're running under cargo-xwin then we need to scrape out its xwin directory
        // and let the linker know where the xwin'd SDK and CRT are.
        let compile_args = self.get_compiler_options("x86_64");
        let linker_flags = self.get_linker_options("x86_64");

        let mut installer = cc::Build::new();
        self.apply_common_cc_options(&mut installer, BuildType::Target);

        let mut cc_cmd = installer
            .target("x86_64-pc-windows-msvc")
            .static_crt(true)
            .flag(&format!("/Fe{}", output_path.display()))
            .output_executable();

        cc_cmd
            .args(&compile_args)
            // See [PathToArg] for why the .to_arg() is here.
            .arg(self.libwdi_src.join("libwdi/installer.c").to_arg())
            // Add the WinAPI libraries we need to link against.
            .args(&[
                "/link",
                "newdev.lib",
                "setupapi.lib",
                "user32.lib",
                "ole32.lib",
                "advapi32.lib",
            ])
            .args(&linker_flags);

        info!("{:?}", cc_cmd);

        let success = cc_cmd
            .current_dir(&self.out_dir)
            .status()
            .unwrap()
            .success();
        if !success {
            panic!("Compiler returned non-zero exit code");
        }
    }

    /// Builds the `installer_arm64.exe` binary which gets embedded into a C header for the
    /// rest of the library.
    ///
    /// Like [make_embedder], this function also contains hacks to use [cc] to compile an
    /// executable instead of a library.
    fn make_installer_arm64(&self)
    {
        // The equivalent msbuild command to build this manually:
        // $MSBUILD libwdi/.msvc/installer_arm64.vcxproj -p:PlatformToolset=143
        // -p:PerferredToolArchitecture=ARM64 -p:Platform=ARM64 -p:Configuration=Release

        // Which runs roughly this command:
        // cl.exe /c /I..\..\msvc /Zi /nologo /W3 /WX- /O1 /GL /D _CRT_SECURE_NO_WARNINGS /D _WIN64
        // /D _WINDLL /D _UNICODE /Gm- /EHsc /MT /GS /fp:precise /Qspectre /Zc:wchar_t /Zc:forScope
        // /Zc:inline /external:W3 /Gd /TC /FC ..\installer.c

        info!("Building installer_arm64...");

        let output_path = self.out_dir.join("installer_arm64.exe");

        // HACK: if we're running under cargo-xwin then we need to scrape out its xwin directory
        // and let the compiler linker know where the xwin'd SDK and CRT are.
        let compile_args = self.get_compiler_options("aarch64");
        let linker_flags = self.get_linker_options("aarch64");

        let mut installer = cc::Build::new();
        self.apply_common_cc_options(&mut installer, BuildType::Target);

        let mut cc_cmd = installer
            .target("aarch64-pc-windows-msvc")
            .static_crt(true)
            .flag(&format!("/Fe{}", output_path.display()))
            .output_executable();

        cc_cmd
            .args(&compile_args)
            // See [PathToArg] for why the .to_arg() is here.
            .arg(self.libwdi_src.join("libwdi/installer.c").to_arg())
            // Add the WinAPI libraries we need to link against.
            .args(&[
                "/link",
                "newdev.lib",
                "setupapi.lib",
                "user32.lib",
                "ole32.lib",
                "advapi32.lib",
            ])
            .args(&linker_flags);

        info!("{:?}", cc_cmd);

        let success = cc_cmd
            .current_dir(&self.out_dir)
            .status()
            .unwrap()
            .success();
        if !success {
            panic!("Compiler returned non-zero exit code");
        }
    }

    /// Runs the host embedder executable built in [make_embedder]. See that function for more
    /// details.
    fn run_embedder(&self)
    {
        info!("Running embedder host binary...");

        let mut cmd = Command::new(self.out_dir.join("embedder.exe"));
        cmd
            .current_dir(&self.libwdi_src.join("libwdi"))
            .arg("embedded.h");
        info!("{:?}", cmd);

        let success = cmd
            .status()
            .unwrap()
            .success();
        if !success {
            panic!("Embedder executable returned non-zero exit code");
        }
    }

    /// Builds the actual libwdi static library (wdi.lib and libwdi.a).
    ///
    /// With [make_embedder], [make_installer_x86_64], and [run_embedder] out of the way, this function
    /// finally uses [cc] only for its intended purpose.
    fn make_lib(&self)
    {
        info!("Building libwdi static library...");

        let lib_srcs: Vec<OsString> = [
            "libwdi.c",
            "libwdi_dlg.c",
            "logging.c",
            "pki.c",
            "tokenizer.c",
            "vid_data.c",
        ]
            .into_iter()
            // See [PathToArg] for why the .to_arg() is here.
            .map(|path| self.libwdi_src.join("libwdi").join(path).to_arg())
            .collect();

        let mut lib = cc::Build::new();
        self.apply_common_cc_options(&mut lib, BuildType::Target);
        lib
            .files(&lib_srcs)
            .compile("wdi");

        println!("cargo:include={}", self.libwdi_src.join("libwdi").to_str().unwrap());
        println!("cargo:rustc-link-lib=shell32");
        println!("cargo:rustc-link-lib=ole32");
    }

    /// This function is only used when the feature "dynamic-bindgen" is enabled, which isn't
    /// recommended.
    fn run_bindgen(&self)
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
    if cfg!(feature = "enable-x86") {
        build.make_installer_x86_64();
    }
    if cfg!(feature = "enable-arm64") {
        build.make_installer_arm64();
    }
    build.run_embedder();
    build.make_lib();

    if cfg!(feature = "dynamic-bindgen") {
        build.run_bindgen();
    }
}
