/// Nitid transpiler binary entry point.
///
/// Reads one or more `.nt` source files, transpiles them to C,
/// writes the generated C source and a CMakeLists.txt to the output
/// directory, and optionally compiles + runs the result.
use std::fs;
use std::path::Path;
use std::process::Command;
use clap::Parser;

/// CLI argument definitions via `clap`.
#[derive(Parser)]
#[command(name = "nitid", version, about = "Yet Another C-derived Language transpiler")]
struct Cli {
    /// Input `.nt` source files.
    #[arg(required = true)]
    files: Vec<String>,

    /// Directory to write generated C sources into.
    #[arg(long, default_value = "c_src")]
    c_dir: String,

    /// Print emitted C to stdout instead of writing files.
    #[arg(long)]
    emit_c: bool,

    /// After transpiling, compile and run the program.
    #[arg(long)]
    run: bool,

    /// C compiler to use (passed to CMake). Defaults to `gcc`.
    #[arg(long)]
    cc: Option<String>,

    /// Output binary path (used with `--run`).
    #[arg(short = 'o')]
    output: Option<String>,
}

/// CLI entry point: parse arguments, transpile files, optionally compile and run.
fn main() -> Result<(), String> {
    let cli = Cli::parse();
    let c_src_dir = &cli.c_dir;

    // Ensure the output directory (and its `runtime/` subdirectory) exist.
    fs::create_dir_all(format!("{}/runtime", c_src_dir))
        .map_err(|e| format!("Failed to create output dir '{}': {}", c_src_dir, e))?;

    let mut all_c_files = Vec::new();
    let mut all_cmake = String::new();
    let mut dangling_count: usize = 0;

    // Transpile each input file.
    for file_path in &cli.files {
        let content = fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read '{}': {}", file_path, e))?;

        let (program, mut c_files, cmake) = nitid::compile(file_path, &content, c_src_dir)?;

        if program.has_dangling {
            dangling_count += 1;
        }

        // Strip runtime files from each file's output (we merge them below).
        c_files.retain(|f| !f.path.contains("runtime/"));
        all_c_files.extend(c_files);
        all_cmake = cmake;
    }

    // Dangling code (statements outside any `fn`) is auto-wrapped in `main`.
    // Having such code in more than one file would produce duplicate `main`
    // definitions — hence the error.
    if dangling_count > 1 {
        return Err("Dangling code in more than one source file \u{2014} cannot auto-generate main".to_string());
    }

    // Copy the Nitid runtime sources (nitid_string) into the output directory.
    let proj_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (src, dst) in [
        (proj_root.join("runtime/nitid_string.h"), format!("{}/runtime/nitid_string.h", c_src_dir)),
        (proj_root.join("runtime/nitid_string.c"), format!("{}/runtime/nitid_string.c", c_src_dir)),
        (proj_root.join("runtime/nitid_string16.h"), format!("{}/runtime/nitid_string16.h", c_src_dir)),
        (proj_root.join("runtime/nitid_string16.c"), format!("{}/runtime/nitid_string16.c", c_src_dir)),
        (proj_root.join("runtime/nitid_string32.h"), format!("{}/runtime/nitid_string32.h", c_src_dir)),
        (proj_root.join("runtime/nitid_string32.c"), format!("{}/runtime/nitid_string32.c", c_src_dir)),
        (proj_root.join("runtime/nitid_array.h"), format!("{}/runtime/nitid_array.h", c_src_dir)),
        (proj_root.join("runtime/nitid_array.c"), format!("{}/runtime/nitid_array.c", c_src_dir)),
    ] {
        let content = fs::read_to_string(&src)
            .map_err(|e| format!("Failed to read runtime file '{}': {}", src.display(), e))?;
        all_c_files.push(nitid::codegen::CFile { path: dst, content });
    }

    // Write every generated C file to disk (or print to stdout).
    for cfile in &all_c_files {
        if let Some(parent) = Path::new(&cfile.path).parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create dir '{}': {}", parent.display(), e))?;
        }
        fs::write(&cfile.path, &cfile.content)
            .map_err(|e| format!("Failed to write '{}': {}", cfile.path, e))?;
        if cli.emit_c {
            println!("--- {} ---", cfile.path);
            println!("{}", cfile.content);
        } else {
            println!("Wrote {}", cfile.path);
        }
    }

    // Write CMakeLists.txt.
    let cmake_path = format!("{}/CMakeLists.txt", c_src_dir);
    fs::write(&cmake_path, &all_cmake)
        .map_err(|e| format!("Failed to write CMakeLists.txt: {}", e))?;
    if cli.emit_c {
        println!("--- {} ---", cmake_path);
        println!("{}", all_cmake);
    } else {
        println!("Wrote {}", cmake_path);
    }

    // Optional: compile and run via CMake.
    if cli.run {
        let _cc = cli.cc.as_deref().unwrap_or("gcc");
        let _binary = cli.output.as_deref().unwrap_or("program");

        // cmake -S <c_src_dir> -B <c_src_dir>/build
        let status = Command::new("cmake")
            .args(["-S", c_src_dir, "-B", &format!("{}/build", c_src_dir)])
            .status()
            .map_err(|e| format!("cmake failed: {}", e))?;
        if !status.success() {
            return Err("cmake configuration failed".to_string());
        }

        // cmake --build <c_src_dir>/build
        let status = Command::new("cmake")
            .args(["--build", &format!("{}/build", c_src_dir)])
            .status()
            .map_err(|e| format!("cmake build failed: {}", e))?;
        if !status.success() {
            return Err("cmake build failed".to_string());
        }

        // Copy binary if output path was specified.
        if let Some(out) = cli.output {
            fs::copy(
                format!("{}/build/{}", c_src_dir, if cfg!(target_os = "linux") { "" } else { "" }),
                &out,
            ).ok();
        }

        let bin_path = format!("{}/build/{}", c_src_dir,
            if cfg!(target_os = "windows") { "program.exe" } else { "program" });

        let status = Command::new(&bin_path)
            .status()
            .map_err(|e| format!("Failed to run '{}': {}", bin_path, e))?;
        if !status.success() {
            return Err(format!("Program exited with code {:?}", status.code()));
        }
    }

    Ok(())
}
