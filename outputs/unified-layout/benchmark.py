"""Run after cargo build --release. Same-layout PNG/base64 pipeline comparison.

The reference uses the previous renderer's usvg/resvg/tiny-skia encoding path
on the *new* SVG, separating pixel-area changes from raster/cache gains.
This excludes filesystem and terminal transfer. Frame preparation is separate.
"""
import json
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[2]
evidence = Path(__file__).resolve().parent
deps = root / "target/release/deps"
with tempfile.TemporaryDirectory(prefix="diagram-benchmark-") as tmp:
    binary = Path(tmp) / "bench"
    cmd = ["rustc", "--edition=2021", "-C", "opt-level=3", str(evidence / "bench.rs"),
           "-L", f"dependency={deps}", "-o", str(binary)]
    for name in ("serde", "serde_json", "resvg", "taffy", "png"):
        lib = max(deps.glob(f"lib{name}-*.rlib"), key=lambda p: p.stat().st_mtime)
        cmd += ["--extern", f"{name}={lib}"]
    subprocess.run(cmd, check=True)
    results = []
    for fixture in ("ci-pipeline", "mechanism"):
        for density in (2, 3):
            output = subprocess.check_output([str(binary), str(evidence / f"{fixture}.json"), str(density)], text=True)
            data = json.loads(output)
            results.append(data)
            print(json.dumps(data), flush=True)
    (evidence / "benchmarks.json").write_text(json.dumps(results, indent=2) + "\n")
