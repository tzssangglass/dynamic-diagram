"""Diagnostic artifact, not a production test or proposed renderer implementation.

Historical baseline harness: its Rust helper targets commit ad20a6f, not the
new layout API. Run in a checkout of that commit after `cargo build --release`
and copying this evidence directory there:
  python3 outputs/layout-performance-audit/reproduce.py --assert-layout
  python3 outputs/layout-performance-audit/reproduce.py --bench

The current baseline intentionally exits 1 for --assert-layout: actual SVG
text boxes overlap or leave the viewport. This is a partial geometry probe;
it does not certify graph edges, icon silhouettes, or animation stability.
For the new renderer use outputs/unified-layout/benchmark.py and cargo test.
"""

import argparse
import json
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--assert-layout", action="store_true")
    parser.add_argument("--bench", action="store_true")
    args = parser.parse_args()
    evidence = Path(__file__).resolve().parent
    root = evidence.parent.parent
    deps = root / "target/release/deps"
    with tempfile.TemporaryDirectory(prefix="dynamic-diagram-audit-") as tmp:
        binary = Path(tmp) / "audit"
        cmd = ["rustc", "--edition=2021", "-C", "opt-level=3",
               str(evidence / "audit.rs"), "-L", f"dependency={deps}",
               "-o", str(binary)]
        for name in ("serde", "serde_json", "resvg"):
            libs = sorted(deps.glob(f"lib{name}-*.rlib"),
                          key=lambda p: p.stat().st_mtime, reverse=True)
            if not libs:
                raise SystemExit("Run cargo build --release first.")
            cmd += ["--extern", f"{name}={libs[0]}"]
        subprocess.run(cmd, check=True)
        failures = 0
        for name in ("ci-pipeline", "mechanism"):
            fixture = str(evidence / f"{name}.json")
            result = subprocess.run([str(binary), "geometry", fixture],
                                    capture_output=True, text=True, check=True)
            data = json.loads(result.stdout)
            print(json.dumps(data, ensure_ascii=False), flush=True)
            failures += bool(data["text_collisions"] or data["text_overflow"])
            if args.bench:
                for scale in (1, 1.5, 2, 3, 4.5):
                    subprocess.run([str(binary), "bench", fixture, str(scale), "96"],
                                   check=True)
        if args.assert_layout and failures:
            raise SystemExit(f"Layout contract failed for {failures} fixtures (baseline defect).")


if __name__ == "__main__":
    main()
