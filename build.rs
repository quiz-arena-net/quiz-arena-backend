use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, ensure};

const PROTO_ROOT: &str = "api/proto";
const DESCRIPTOR_SET: &str = "quiz-arena-backend.fds.bin";

fn main() -> anyhow::Result<()> {
    // connectrpc-build's own rerun directives are disabled below, so these
    // are the only rebuild triggers.
    println!("cargo:rerun-if-changed={PROTO_ROOT}");
    println!("cargo:rerun-if-changed=buf.yaml");
    println!("cargo:rerun-if-changed=buf.lock");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").context("OUT_DIR is unset")?);
    let descriptor_set = build_descriptor_set(&out_dir)?;

    // Codegen selects files by module-relative name as stored in the
    // descriptor set (e.g. `quiz_arena/template/v1/template.proto`).
    let mut protos = collect_protos(Path::new(PROTO_ROOT))?
        .iter()
        .map(|path| path.strip_prefix(PROTO_ROOT).map(Path::to_path_buf))
        .collect::<Result<Vec<_>, _>>()
        .context("collected proto outside PROTO_ROOT")?;
    protos.sort();

    connectrpc_build::Config::new()
        .files(&protos)
        .descriptor_set(descriptor_set)
        .include_file("_connectrpc.rs")
        // Embedded by src/proto.rs to back gRPC server reflection.
        .emit_descriptor_set(DESCRIPTOR_SET)
        // The crate would watch the OUT_DIR descriptor set we rewrite every
        // run and retrigger each build.
        .emit_rerun_directives(false)
        .compile()?;

    Ok(())
}

/// Compile the buf workspace to a `FileDescriptorSet` in `out_dir`, letting
/// buf resolve BSR deps (protovalidate) per `buf.yaml`/`buf.lock`.
///
/// buf is invoked directly because [`connectrpc_build::Config::use_buf`]
/// cannot target a module below the workspace root: it reuses `files()` both
/// as `buf build --path` filters (workspace-relative) and as codegen
/// selectors (module-relative).
fn build_descriptor_set(out_dir: &Path) -> anyhow::Result<PathBuf> {
    let path = out_dir.join("buf-workspace.fds.bin");
    let output = Command::new("buf")
        .args(["build", "--as-file-descriptor-set", "-o"])
        .arg(&path)
        .output()
        .context("failed to spawn buf (is it on PATH?)")?;
    ensure!(
        output.status.success(),
        "buf build failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(path)
}

/// Recursively collect `.proto` files under `dir` as filesystem paths.
fn collect_protos(dir: &Path) -> io::Result<Vec<PathBuf>> {
    fs::read_dir(dir)?.try_fold(Vec::new(), |mut protos, entry| {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            protos.extend(collect_protos(&path)?);
        } else if path.extension().is_some_and(|ext| ext == "proto") {
            protos.push(path);
        }
        Ok(protos)
    })
}
