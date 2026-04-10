#!/usr/bin/env python3
"""CV triage over the v1_local_baseline summary CSVs.

Reads phase1_lenient_summary.csv, phase1_strict_summary.csv and
phase2_summary.csv under results/locked/v1_local_baseline/, filters rows
with cv > THRESHOLD (default 0.15), and emits cv_triage.md with a
disposition column (keep/rerun/exclude) and a short rationale for each
flagged datapoint.

Default policy: "keep with footnote". The operator (Filipe) reviews the
emitted file and can manually change disposition to `exclude` (row
dropped from the article plots) or `rerun` (row must be regenerated
before the snapshot is signed off).

Usage:
    python3 scripts/cv_triage.py [--threshold 0.15] [--baseline-dir PATH]

Output:
    <baseline-dir>/cv_triage.md
"""

from __future__ import annotations

import argparse
import csv
import sys
from pathlib import Path

DEFAULT_THRESHOLD = 0.15
SUMMARY_FILES = [
    ("Phase 1 (lenient)", "phase1_lenient_summary.csv"),
    ("Phase 1 (strict)", "phase1_strict_summary.csv"),
    ("Phase 2 (Docker)", "phase2_summary.csv"),
]


def flag_rows(summary_path: Path, threshold: float) -> list[dict]:
    if not summary_path.exists():
        return []
    rows = []
    with summary_path.open(newline="") as f:
        reader = csv.DictReader(f)
        for row in reader:
            try:
                cv = float(row.get("cv", "0") or "0")
            except ValueError:
                continue
            if cv <= threshold:
                continue
            try:
                mean = float(row.get("wall_clock_mean", "0") or "0")
            except ValueError:
                mean = 0.0
            rows.append(
                {
                    "benchmark": row.get("benchmark", ""),
                    "input_size": row.get("input_size", ""),
                    "mode": row.get("mode", ""),
                    "workers": row.get("workers", ""),
                    "reps": row.get("repetitions", ""),
                    "all_correct": row.get("all_correct", ""),
                    "mean_s": mean,
                    "cv": cv,
                }
            )
    rows.sort(key=lambda r: (-r["cv"], r["benchmark"], r["input_size"]))
    return rows


def default_disposition(row: dict, threshold: float) -> tuple[str, str]:
    cv = row["cv"]
    mean = row["mean_s"]
    correct = row["all_correct"].lower() == "true"
    if not correct:
        return "exclude", "all_correct=false; row already invalid"
    if mean < 0.010:
        return (
            "keep",
            f"tiny wall-clock ({mean*1000:.2f} ms); CV {cv:.3f} is timer noise, not variance",
        )
    if cv > 0.30:
        return (
            "rerun",
            f"CV {cv:.3f} > 0.30 with mean {mean:.3f}s — investigate before keeping",
        )
    return (
        "keep",
        f"CV {cv:.3f} above {threshold:.2f} threshold but < 0.30; keep with footnote in the article",
    )


def render(baseline_dir: Path, threshold: float, buckets: dict[str, list[dict]]) -> str:
    lines: list[str] = []
    lines.append("# CV Triage — v1_local_baseline")
    lines.append("")
    lines.append(f"- **Threshold:** CV > {threshold:.2f}")
    lines.append(f"- **Baseline dir:** `{baseline_dir}`")
    total = sum(len(rows) for rows in buckets.values())
    lines.append(f"- **Flagged datapoints:** {total}")
    lines.append("")
    lines.append(
        "Default dispositions are automatic and conservative. Review each row "
        "manually; change `keep` to `exclude` to drop from the article plots, "
        "or to `rerun` to regenerate before signing off the snapshot."
    )
    lines.append("")

    for label, rows in buckets.items():
        lines.append(f"## {label}")
        lines.append("")
        if not rows:
            lines.append(f"_No datapoints with CV > {threshold:.2f}._")
            lines.append("")
            continue
        lines.append(
            "| Benchmark | Size | Mode | Workers | Reps | Correct | Mean (s) | CV | Disposition | Reason |"
        )
        lines.append("|---|---|---|---|---|---|---|---|---|---|")
        for row in rows:
            disp, reason = default_disposition(row, threshold)
            lines.append(
                "| {benchmark} | {input_size} | {mode} | {workers} | {reps} | {all_correct} | {mean_s:.6f} | {cv:.4f} | {disp} | {reason} |".format(
                    **row, disp=disp, reason=reason
                )
            )
        lines.append("")

    lines.append("---")
    lines.append("")
    lines.append(
        "Excluded rows (if any) should be listed in the article as a "
        "'datapoints descartados por variancia' footnote; rerun rows block the "
        "snapshot sign-off until regenerated."
    )
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description="CV triage over v1_local_baseline")
    parser.add_argument(
        "--threshold",
        type=float,
        default=DEFAULT_THRESHOLD,
        help=f"CV threshold above which to flag a datapoint (default: {DEFAULT_THRESHOLD})",
    )
    parser.add_argument(
        "--baseline-dir",
        type=Path,
        default=None,
        help="Baseline directory (default: <repo>/results/locked/v1_local_baseline)",
    )
    args = parser.parse_args()

    if args.baseline_dir is None:
        script_dir = Path(__file__).resolve().parent
        baseline_dir = script_dir.parent / "results" / "locked" / "v1_local_baseline"
    else:
        baseline_dir = args.baseline_dir.resolve()

    if not baseline_dir.is_dir():
        print(f"ERROR: {baseline_dir} does not exist", file=sys.stderr)
        return 1

    buckets: dict[str, list[dict]] = {}
    for label, filename in SUMMARY_FILES:
        buckets[label] = flag_rows(baseline_dir / filename, args.threshold)

    report = render(baseline_dir, args.threshold, buckets)
    out_path = baseline_dir / "cv_triage.md"
    out_path.write_text(report, encoding="utf-8")

    total = sum(len(rows) for rows in buckets.values())
    print(f"Wrote {out_path} ({total} flagged datapoints)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
