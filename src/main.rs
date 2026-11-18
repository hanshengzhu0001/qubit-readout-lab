use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use nexosim_control_lab::runner::{run_cases, sweep_cases};
use nexosim_control_lab::{Config, Report};
use serde::Serialize;

#[derive(Parser)]
#[command(about = "Run and verify a synthetic control-timing simulation")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Run {
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Sweep {
        #[arg(long, default_value_t = 500)]
        jobs: u32,
        #[arg(long, default_value_t = 2)]
        parallel: usize,
        #[arg(long)]
        out: PathBuf,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let (reports, out) = match Cli::parse().command {
        Command::Run { config, out } => {
            let input =
                File::open(&config).with_context(|| format!("opening {}", config.display()))?;
            let name = config.file_stem().and_then(|s| s.to_str()).unwrap_or("run");
            let config = serde_json::from_reader(BufReader::new(input))?;
            (run_cases(vec![(name.to_owned(), config)], 1).await?, out)
        }
        Command::Sweep {
            jobs,
            parallel,
            out,
        } => (run_cases(sweep_cases(jobs), parallel).await?, out),
    };
    // Each invocation creates a fresh output directory, so runs cannot mix files.
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(&out)
        .with_context(|| format!("creating fresh output directory {}", out.display()))?;
    if let [report] = reports.as_slice() {
        save_report(&out, report)?;
        println!("{}", serde_json::to_string_pretty(&report.metrics)?);
    } else {
        #[derive(Serialize)]
        struct Summary<'a> {
            name: &'a str,
            config: &'a Config,
            metrics: &'a nexosim_control_lab::Metrics,
            reference_checked: bool,
        }
        let summary: Vec<_> = reports
            .iter()
            .map(|r| Summary {
                name: &r.name,
                config: &r.config,
                metrics: &r.metrics,
                reference_checked: r.reference_checked,
            })
            .collect();
        for report in &reports {
            let case_dir = out.join(&report.name);
            fs::create_dir(&case_dir)?;
            save_report(&case_dir, report)?;
        }
        write_json(&out.join("sweep.json"), &summary)?;
        println!("{} cases matched the reference", reports.len());
    }
    Ok(())
}

fn write_json(path: &Path, data: &impl Serialize) -> Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    serde_json::to_writer_pretty(&mut file, data)?;
    writeln!(file)?;
    file.flush()?;
    Ok(())
}

fn save_report(out: &Path, report: &Report) -> Result<()> {
    write_json(&out.join("report.json"), report)?;
    let mut csv = BufWriter::new(File::create(out.join("trace.csv"))?);
    writeln!(csv, "id,time_us,phase,decoder_in_system,queue_wait_us")?;
    for t in &report.trace {
        writeln!(
            csv,
            "{},{},{},{},{}",
            t.id,
            t.time_us,
            t.phase.as_str(),
            t.decoder_in_system
                .map_or_else(String::new, |v| v.to_string()),
            t.queue_wait_us.map_or_else(String::new, |v| v.to_string())
        )?;
    }
    csv.flush()?;
    let mut gauges = String::new();
    let metrics = &report.metrics;
    for (name, value) in [
        ("generated", u64::from(metrics.generated)),
        ("on_time", metrics.on_time as u64),
        ("deadline_misses", metrics.deadline_misses as u64),
        ("capacity_drops", metrics.capacity_drops as u64),
        ("deadline_rejections", metrics.deadline_rejections as u64),
        ("decoder_busy_microseconds", metrics.decoder_busy_us),
        ("link_payload_bytes", metrics.link_payload_bytes),
    ] {
        writeln!(
            &mut gauges,
            "# TYPE control_lab_{name} gauge\ncontrol_lab_{name} {value}"
        )?;
    }
    fs::write(out.join("metrics.prom"), gauges)?;
    Ok(())
}
