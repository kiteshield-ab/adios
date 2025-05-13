use std::{collections::HashMap, io::Read};

use anyhow::anyhow;
use clap::Parser;
use cli::Mode;
use clio::Input;
use probe_rs::{MemoryInterface, Permissions};

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error,regkirurg=debug")).init();

    let mut args = cli::Args::parse();

    args.svd
        .push(Input::new("../../svds/MIMXRT1189_cm33.svd").unwrap());

    let register_regex = args.mode.registers();
    let mut regs_to_read = HashMap::new();
    for mut svd_file in args.svd {
        let mut svd_as_string = String::new();
        svd_file.read_to_string(&mut svd_as_string).unwrap();
        let device = svd_parser::parse(&svd_as_string).unwrap();
        regdoctor::for_each_register(device, |(ri, address)| {
            let identifier = ri.identifier();
            if !register_regex.is_match(&identifier) {
                return;
            }
            match regs_to_read.insert(identifier, (ri, address)) {
                Some(previous) => {
                    log::info!(
                        "Identifier collision: [{}] overwrites [{}]",
                        address,
                        previous.1,
                    );
                }
                None => {}
            }
        });
    }
    match args.mode {
        Mode::Filter { .. } => {
            for (identifier, (register_info, address)) in regs_to_read.into_iter() {
                let Some(bit_width) = register_info.inner.properties.size else {
                    log::warn!("{identifier}: unknown register size, skipping");
                    continue;
                };
                println!("{identifier} at 0x{address:08x}, {bit_width} wide");
            }
        }
        Mode::ReadWithProbe {
            chip, probe_serial, ..
        } => {
            let probes = probe_rs::probe::list::Lister::new().list_all();
            let probe_serial = probe_serial.as_ref();
            let probe = probes
                .into_iter()
                .find(|p| {
                    probe_serial
                        .map(|serial| p.serial_number.as_ref() == Some(serial))
                        .unwrap_or(true)
                })
                .ok_or(anyhow!("Probe not found"))?
                .open()?;

            let mut session = probe.attach(chip, Permissions::default())?;
            let mut core = session.core(0)?;

            for (identifier, (register_info, address)) in regs_to_read.into_iter() {
                let Some(bit_width) = register_info.inner.properties.size else {
                    log::warn!("{identifier}: unknown register size, skipping");
                    continue;
                };
                let value = match bit_width {
                    8 => core.read_word_8(address)? as u32,
                    16 => core.read_word_16(address)? as u32,
                    32 => core.read_word_32(address)?,
                    bit_width => {
                        log::warn!(
                            "{identifier}: unsupported register size: {bit_width}, skipping"
                        );
                        continue;
                    }
                };
                log::info!("{identifier}: 0x{value:08x}");
                let register = register_info.decode_value(value as _);
                log::debug!("{register:x?}");
            }
        }
    }

    Ok(())
}

mod cli {
    use clap::{Parser, Subcommand};
    use clio::Input;
    use regex::Regex;

    #[derive(Subcommand, Debug)]
    pub enum Mode {
        Filter {
            registers: Regex,
        },
        ReadWithProbe {
            registers: Regex,

            #[arg(long)]
            chip: String,

            #[arg(long)]
            probe_serial: Option<String>,
        },
    }

    impl Mode {
        pub fn registers(&self) -> Regex {
            match self {
                Mode::Filter { registers } => registers.clone(),
                Mode::ReadWithProbe { registers, .. } => registers.clone(),
            }
        }
    }

    #[derive(Parser, Debug)]
    pub struct Args {
        #[clap(subcommand)]
        pub mode: Mode,

        #[arg(short = 's', long, value_parser)]
        pub svd: Vec<Input>,
    }
}
