#!/usr/bin/env python3
"""Compare nano-web benchmark runs produced by bench/run.sh.

    bench/compare.py <label>             # show one run
    bench/compare.py <baseline> <new>    # show new vs baseline with deltas

Reads bench/results/<label>/<scenario>.json (oha JSON output). Latencies are
reported in microseconds, throughput in requests/sec, transfer in MiB/s.
"""

import json
import sys
from pathlib import Path

RESULTS = Path(__file__).resolve().parent / "results"


def load(label):
    d = RESULTS / label
    if not d.is_dir():
        sys.exit(f"no results for label '{label}' (looked in {d})")
    out = {}
    for f in sorted(d.glob("*.json")):
        text = f.read_text()
        if not text.strip():
            print(f"  ! skipping empty {f.name} (run still in progress?)", file=sys.stderr)
            continue
        try:
            data = json.loads(text)
        except json.JSONDecodeError:
            print(f"  ! skipping malformed {f.name}", file=sys.stderr)
            continue
        s = data.get("summary", {})
        pct = data.get("latencyPercentiles", {})
        out[f.stem] = {
            "rps": s.get("requestsPerSec", 0.0),
            "p50": pct.get("p50", 0.0) * 1e6,
            "p90": pct.get("p90", 0.0) * 1e6,
            "p99": pct.get("p99", 0.0) * 1e6,
            "mibs": s.get("sizePerSec", 0.0) / (1024 * 1024),
            "codes": data.get("statusCodeDistribution", {}),
        }
    return out


def fmt(n, w=12):
    return f"{n:>{w},.1f}"


def delta(new, old):
    if old == 0:
        return "   n/a"
    pct = (new - old) / old * 100
    return f"{pct:+6.1f}%"


def show_one(label):
    runs = load(label)
    print(f"\n  {label}\n")
    hdr = f"  {'scenario':<16}{'req/s':>12}{'p50 µs':>11}{'p90 µs':>11}{'p99 µs':>11}{'MiB/s':>11}"
    print(hdr)
    print("  " + "-" * (len(hdr) - 2))
    for name, m in runs.items():
        print(f"  {name:<16}{fmt(m['rps'])}{fmt(m['p50'],11)}{fmt(m['p90'],11)}"
              f"{fmt(m['p99'],11)}{fmt(m['mibs'],11)}")


def show_diff(base_label, new_label):
    base, new = load(base_label), load(new_label)
    print(f"\n  {new_label} vs {base_label}  (Δ = new relative to baseline)\n")
    hdr = (f"  {'scenario':<16}{'req/s':>12}{'Δrps':>8}"
           f"{'p50 µs':>10}{'p99 µs':>10}{'Δp99':>8}")
    print(hdr)
    print("  " + "-" * (len(hdr) - 2))
    for name in sorted(set(base) | set(new)):
        b, n = base.get(name), new.get(name)
        if not n:
            print(f"  {name:<16}  (missing in {new_label})")
            continue
        if not b:
            print(f"  {name:<16}{fmt(n['rps'])}  (new)")
            continue
        # p99 lower is better, so invert the delta sign for readability
        p99_delta = delta(b["p99"], n["p99"])
        print(f"  {name:<16}{fmt(n['rps'])}{delta(n['rps'],b['rps']):>8}"
              f"{fmt(n['p50'],10)}{fmt(n['p99'],10)}{p99_delta:>8}")
    print("\n  Δrps: higher is better. Δp99: higher is better (latency dropped).\n")


def main():
    args = sys.argv[1:]
    if len(args) == 1:
        show_one(args[0])
    elif len(args) == 2:
        show_diff(args[0], args[1])
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
