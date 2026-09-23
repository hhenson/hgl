"""Reproduce the isolated experiment; no source expectations are regenerated."""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import tempfile
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def run(args, cwd=HERE, environment=None):
    p = subprocess.run(args, cwd=cwd, env=environment, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    if p.returncode:
        raise RuntimeError(f"Command failed: {args}\n{p.stdout}")
    return p.stdout


def cargo(*args):
    return run(["cargo", *args, "--manifest-path", str(HERE / "Cargo.toml")])


def validate_generated(output):
    # Cargo options precede the executable's arguments.
    emitted = output / "emitted"
    run(["cargo", "run", "--quiet", "--locked", "--", "emit", str(emitted)])
    with tempfile.TemporaryDirectory(prefix="hgl-emitted-spike-") as directory:
        target = Path(directory)
        (target / "src").mkdir()
        (target / "tests").mkdir()
        shutil.copyfile(ROOT / "rust-toolchain.toml", target / "rust-toolchain.toml")
        dependencies = "\n".join(f'{name} = {{ path = {json.dumps(str(ROOT / "crates" / name))} }}' for name in ["hgl-types", "hgl-store", "hgl-kernel", "hgl-describe", "hgl-nested", "hgl-testkit", "hgl-plan"])
        (target / "Cargo.toml").write_text('[package]\nname = "hgl-emitted-spike"\nversion = "0.0.0"\nedition = "2024"\n[workspace]\n[dependencies]\n' + dependencies + '\n')
        shutil.copyfile(HERE / "runtime-tests.rs.in", target / "tests/runtime.rs")
        fixture = ROOT / "crates/hgl-describe/tests/recursive_support"
        shutil.copytree(fixture, target / "tests/recursive_support")
        shutil.copyfile(ROOT / "crates/hgl-describe/tests/recursive.rs", target / "tests/recursive.rs")
        path = target / "tests/recursive_support/mod.rs"
        text = path.read_text()
        start = text.index("fn shape() -> TsType {")
        end = text.index("\nfn owner_inputs()", start)
        text = text[:start] + '''fn shape() -> TsType {
    let TsType::Reference(shape) = hgl_emitted_spike::shape_forward() else {
        panic!("emitted reference shape expected")
    };
    *shape
}
''' + text[end:]
        path.write_text(text)
        variants = sorted(emitted.glob("*.rs"))
        assert len(variants) == 4, variants
        for variant in variants:
            shutil.copyfile(variant, target / "src/lib.rs")
            log = run(["cargo", "test", "--offline"], cwd=target)
            (output / f"{variant.stem}-tests.log").write_text(log)
        return [v.stem for v in variants]


def measurements(output):
    builds = []
    environment = os.environ.copy()
    environment["RUSTC_WRAPPER"] = ""
    environment["RUSTC_WORKSPACE_WRAPPER"] = ""
    variants = {"hand": "", "logos": "logos", "chumsky": "chumsky", "rowan": "rowan", "diagnostics": "codespan-reporting", "emitter": "quote,proc-macro2,syn,prettyplease"}
    for variant, features in variants.items():
        for profile in ["debug", "release"]:
            for repetition in range(3):
                with tempfile.TemporaryDirectory(prefix="hgl-spike-build-") as directory:
                    cmd = ["cargo", "build", "--offline", "--locked", "--no-default-features", "--target-dir", directory]
                    if features: cmd += ["--features", features]
                    if profile == "release": cmd += ["--release"]
                    start = time.perf_counter()
                    run(cmd, environment=environment)
                    builds.append({"variant": variant, "profile": profile, "repetition": repetition, "seconds": time.perf_counter() - start})
                    print(json.dumps(builds[-1]), flush=True)
    (output / "builds.json").write_text(json.dumps(builds, indent=2) + "\n")
    run(["cargo", "build", "--release", "--offline", "--locked"])
    executions = []
    for repetition in range(7):
        text = run([str(HERE / "target/release/hgl-compiler-tools-spike")])
        for row in csv.DictReader(text.splitlines()):
            executions.append({**row, "repetition": repetition})
    (output / "execution.json").write_text(json.dumps(executions, indent=2) + "\n")
    return {name: statistics.median(int(row["nanoseconds"]) / int(row["iterations"]) for row in executions if row["case"] == name) for name in sorted({row["case"] for row in executions})}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--measure", action="store_true")
    parser.add_argument("--reference", type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    output = args.output.resolve()
    (output / "candidate-tests.log").write_text(cargo("test", "--locked"))
    (output / "baseline-tests.log").write_text(cargo("test", "--locked", "--no-default-features"))
    result = {"rustc": run(["rustc", "--version"]).strip(), "platform": platform.system(), "architecture": platform.machine(), "emission_variants": validate_generated(output)}
    result["experiment_sha256"] = {str(p.relative_to(HERE)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted([HERE / "Cargo.toml", HERE / "Cargo.lock", HERE / "run.py", HERE / "runtime-tests.rs.in", *HERE.glob("src/*.rs")])}
    result["corpus_sha256"] = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted((HERE / "corpus").glob("*.hgl"))}
    if args.reference:
        reference = []
        for name, accepted in [("valid", True), ("generic", True), ("normalization", False), ("recovery", False), ("type_error", False)]:
            p = subprocess.run([str(args.reference), "check", str(HERE / f"corpus/{name}.hgl")], text=True, capture_output=True)
            assert (p.returncode == 0) == accepted, (name, p.stdout, p.stderr)
            reference.append({"case": name, "accepted": accepted})
        result["upstream_arithmetic"] = run([str(args.reference), "test", str(HERE / "corpus/runtime.hgl")]).strip()
        result["upstream"] = reference
        result["upstream_binary_sha256"] = hashlib.sha256(args.reference.read_bytes()).hexdigest()
    if args.measure: result["median_ns"] = measurements(output)
    (output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
