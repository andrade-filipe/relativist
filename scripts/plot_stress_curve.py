#!/usr/bin/env python3
"""D-014 stress-curve plot generator (TASK-0705).

Reads `aggregated.csv` (post-TASK-0703 schema) and emits IEEE-quality
PDFs to `--output-dir`:

  <workload>_walltime.pdf   — log-log wall_time vs N for 4 worker
                              counts, 2 envs.
  <workload>_mips.pdf       — log-log MIPS vs N.
  <workload>_vmrss.pdf      — log-log vmrss_peak_mb vs N.
  summary_walls.pdf         — bar chart of N_max per (workload, env, W)
                              with stop_reason colour.

Total: up to 9 metric PDFs (3 metrics × 3 workloads) + 1 summary = 10.
Empty-CSV → exit 2; missing required column → exit 1; otherwise 0.

Dependencies: matplotlib, pandas, numpy. Same versions as
`scripts/cv_triage.py`.

Usage:
    python3 scripts/plot_stress_curve.py \
        --input results/locked/v2_stress_curve_<DATE>/aggregated.csv \
        --output-dir results/locked/v2_stress_curve_<DATE>/figures
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

# Required CSV columns the script reads by name. Order does NOT matter
# (we look up by name). Missing → exit 1.
REQUIRED_COLUMNS = [
    "workload",
    "env",
    "workers",
    "n",
    "rep",
    "wall_seconds",
    "mips",
    "vmrss_peak_mb",
    "vmrss_current_end_mb",
    "stop_reason",
    "cv_above_gate",
]

# Optional columns we may also try to read; tolerated if absent.
OPTIONAL_COLUMNS = ["benchmark", "input_size"]

# 4 colourblind-safe colours from matplotlib `tab10` (indices 0,2,4,6).
WORKER_COLORS = {1: "#1f77b4", 2: "#2ca02c", 4: "#9467bd", 8: "#8c564b"}
WORKER_MARKERS = {1: "o", 2: "s", 4: "^", 8: "D"}
ENV_LINESTYLE = {"in_process": "-", "docker_tcp": "--"}
STOP_REASON_COLOR = {
    "WallTimeExceeded": "#1f77b4",
    "MemoryExceeded": "#d62728",
    "Oom": "#9467bd",
    "": "#2ca02c",  # completed without trip
}


def load_aggregated(path: Path):
    """Load the aggregated CSV. Exits with code 1 if a required column
    is missing, code 2 if the data set is empty (header only)."""
    try:
        import pandas as pd
    except ImportError as e:
        print(f"ERROR: pandas required: {e}", file=sys.stderr)
        sys.exit(1)
    if not path.exists():
        print(f"ERROR: input CSV not found: {path}", file=sys.stderr)
        sys.exit(1)
    df = pd.read_csv(path)
    if df.empty:
        print(f"ERROR: CSV {path} is empty (header only)", file=sys.stderr)
        sys.exit(2)
    for col in REQUIRED_COLUMNS:
        if col not in df.columns:
            print(
                f"ERROR: required column '{col}' missing from {path}",
                file=sys.stderr,
            )
            sys.exit(1)
    return df


def setup_axes(ax, xlabel: str, ylabel: str, log_xy: bool, column_width_in: float):
    ax.set_xlabel(xlabel, fontsize=9)
    ax.set_ylabel(ylabel, fontsize=9)
    ax.tick_params(labelsize=8)
    if log_xy:
        ax.set_xscale("log")
        ax.set_yscale("log")
    ax.grid(True, which="both", linestyle=":", alpha=0.4)


def plot_metric(
    df,
    workload: str,
    metric: str,
    ylabel: str,
    out_path: Path,
    column_width_in: float,
    dpi: int,
):
    """Emit a single log-log PDF for `<workload>_<metric>.pdf`."""
    import matplotlib.pyplot as plt
    import numpy as np

    sub = df[df["workload"] == workload]
    if sub.empty:
        return
    fig, ax = plt.subplots(figsize=(column_width_in, 2.5), dpi=dpi)
    for env, group_env in sub.groupby("env"):
        ls = ENV_LINESTYLE.get(env, "-")
        for w, group_w in group_env.groupby("workers"):
            color = WORKER_COLORS.get(int(w), "#666666")
            marker = WORKER_MARKERS.get(int(w), "x")
            grouped = group_w.groupby("n")[metric]
            xs = sorted(grouped.groups.keys())
            ys_geo = []
            ys_std = []
            cv_flagged = False
            for x in xs:
                vals = grouped.get_group(x).values
                vals_pos = vals[vals > 0]
                if len(vals_pos) == 0:
                    ys_geo.append(np.nan)
                    ys_std.append(0.0)
                    continue
                # Geometric mean for log-axis data.
                ys_geo.append(float(np.exp(np.log(vals_pos).mean())))
                ys_std.append(float(np.std(vals_pos)))
                cv_flag_col = group_w[group_w["n"] == x]["cv_above_gate"]
                if cv_flag_col.any():
                    cv_flagged = True
            label = f"{env} W={w}{'*' if cv_flagged else ''}"
            ax.errorbar(
                xs,
                ys_geo,
                yerr=ys_std,
                color=color,
                linestyle=ls,
                marker=marker,
                label=label,
                markersize=4,
                linewidth=1,
                capsize=2,
            )
    setup_axes(ax, "N (agents)", ylabel, log_xy=True, column_width_in=column_width_in)
    ax.legend(fontsize=7, loc="best")
    fig.savefig(out_path, bbox_inches="tight", pad_inches=0.02)
    plt.close(fig)


def plot_summary(df, out_path: Path, column_width_in: float, dpi: int):
    """Bar chart: N_max per (workload, env, W) coloured by stop_reason."""
    import matplotlib.pyplot as plt

    if df.empty:
        return
    grouped = df.groupby(["workload", "env", "workers"])
    labels = []
    values = []
    colors = []
    for (wl, env, w), sub in grouped:
        n_max = int(sub["n"].max())
        # Stop reason on the row with the largest N (the trip row, if any).
        max_row = sub.loc[sub["n"].idxmax()]
        sr = max_row.get("stop_reason", "")
        if isinstance(sr, float):  # NaN for blank
            sr = ""
        labels.append(f"{wl[:6]}/{env[:5]}/W{w}")
        values.append(n_max)
        colors.append(STOP_REASON_COLOR.get(sr, "#888888"))
    fig, ax = plt.subplots(figsize=(column_width_in, 3.0), dpi=dpi)
    ax.bar(range(len(labels)), values, color=colors)
    ax.set_xticks(range(len(labels)))
    ax.set_xticklabels(labels, rotation=60, ha="right", fontsize=7)
    ax.set_ylabel("N_max reached", fontsize=9)
    ax.set_yscale("log")
    ax.grid(True, axis="y", linestyle=":", alpha=0.4)
    ax.tick_params(labelsize=8)
    fig.savefig(out_path, bbox_inches="tight", pad_inches=0.02)
    plt.close(fig)


def main():
    parser = argparse.ArgumentParser(description="D-014 stress-curve plotter (TASK-0705)")
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--workloads", default="")
    parser.add_argument("--dpi", type=int, default=300)
    parser.add_argument("--column-width-in", type=float, default=3.5)
    parser.add_argument("--no-summary", action="store_true")
    args = parser.parse_args()

    args.output_dir.mkdir(parents=True, exist_ok=True)

    df = load_aggregated(args.input)

    if args.workloads:
        wl_filter = [w.strip() for w in args.workloads.split(",") if w.strip()]
        df = df[df["workload"].isin(wl_filter)]
        if df.empty:
            print(
                f"ERROR: no rows after workload filter {wl_filter}",
                file=sys.stderr,
            )
            sys.exit(2)

    workloads = sorted(df["workload"].dropna().unique().tolist())
    metrics = [
        ("wall_seconds", "wall time (s)", "walltime"),
        ("mips", "MIPS", "mips"),
        ("vmrss_peak_mb", "VmRSS peak (MiB)", "vmrss"),
    ]
    for wl in workloads:
        for col, ylabel, suffix in metrics:
            out = args.output_dir / f"{wl}_{suffix}.pdf"
            plot_metric(
                df, wl, col, ylabel, out, args.column_width_in, args.dpi
            )

    if not args.no_summary:
        plot_summary(
            df,
            args.output_dir / "summary_walls.pdf",
            args.column_width_in,
            args.dpi,
        )

    return 0


if __name__ == "__main__":
    sys.exit(main())
