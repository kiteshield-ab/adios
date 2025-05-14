use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
};

use anyhow::anyhow;
use clap::Parser;
use cli::Mode;
use clio::Input;
use probe_rs::{MemoryInterface, Permissions};
use regdoctor::Register;
use regex::Regex;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let mut args = cli::Args::parse();

    // TODO: unhardcode IMRT118x svd maybe
    args.svd
        .push(Input::new("../../svds/MIMXRT1189_cm33.svd").unwrap());

    let register_include_filter = args.mode.registers();
    let register_exclude_filter = args.mode.exclude();
    let mut filtered_registers = HashMap::new();
    for mut svd_file in args.svd {
        let mut svd_as_string = String::new();
        svd_file.read_to_string(&mut svd_as_string).unwrap();
        let device = svd_parser::parse(&svd_as_string).unwrap();
        regdoctor::for_each_register(device, |(ri, address)| {
            let identifier = ri.identifier();
            if !register_include_filter.is_match(&identifier) {
                return;
            }
            if register_exclude_filter
                .as_ref()
                .map(|v| v.is_match(&identifier))
                .unwrap_or(false)
            {
                return;
            }
            match filtered_registers.insert(identifier, (ri, address)) {
                Some(previous) => {
                    log::warn!(
                        "Identifier collision: [{}] overwrites [{}]",
                        address,
                        previous.1,
                    );
                }
                None => {}
            }
        });
    }

    let mut filtered_registers: Vec<_> = filtered_registers.into_iter().collect();
    filtered_registers.sort_by(|(l, _), (r, _)| l.cmp(r));

    match args.mode {
        Mode::DryRun { .. } => {
            for (identifier, (register_info, address)) in filtered_registers.into_iter() {
                let Some(bit_width) = register_info.inner.properties.size else {
                    log::warn!("{identifier}: unknown register size, skipping");
                    continue;
                };
                if let 8 | 16 | 32 | 64 = bit_width {
                    println!("{identifier} at 0x{address:08x}, {bit_width} wide");
                } else {
                    log::warn!("{identifier}: unsupported register size: {bit_width}, skipping");
                }
            }
        }
        Mode::Read {
            mut output,
            chip,
            probe_serial,
            ..
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

            for (identifier, (register_info, address)) in filtered_registers.into_iter() {
                let Some(bit_width) = register_info.inner.properties.size else {
                    log::warn!("{identifier}: unknown register size, skipping");
                    continue;
                };
                write!(output, "{identifier}: ")?;
                match bit_width {
                    8 => {
                        writeln!(output, "0x{:02x}", core.read_word_8(address)?)?;
                    }
                    16 => {
                        writeln!(output, "0x{:04x}", core.read_word_16(address)?)?;
                    }
                    32 => {
                        writeln!(output, "0x{:08x}", core.read_word_32(address)?)?;
                    }
                    64 => {
                        writeln!(output, "0x{:016x}", core.read_word_64(address)?)?;
                    }
                    bit_width => {
                        log::warn!(
                            "{identifier}: unsupported register size: {bit_width}, skipping"
                        );
                        continue;
                    }
                };
            }
        }
        Mode::Show { input, .. } => {
            let input_path = input.path().clone();
            let input = parse_input(input)?;
            for (identifier, (register_info, ..)) in filtered_registers.into_iter() {
                let Some(&input_value) = input.get(&identifier) else {
                    log::warn!(
                        "{identifier}: identifier absent in input (path: {})",
                        input_path
                    );
                    continue;
                };
                let register = register_info.decode_value(input_value);
                println!("{register:#x?}");
            }
        }
        Mode::Compare { left, right, .. } => {
            let left = parse_input(left)?;
            let right = parse_input(right)?;
            for (identifier, (register_info, ..)) in filtered_registers.into_iter() {
                print!("{identifier}: ");
                match (left.get(&identifier), right.get(&identifier)) {
                    (None, None) => {
                        println!("missing in both");
                    }
                    (None, Some(&value)) => {
                        let register = register_info.decode_value(value);
                        let diff = register.diff_from_nothing();
                        println!("\n{diff}");
                    }
                    (Some(&value), None) => {
                        let register = register_info.decode_value(value);
                        let diff = register.diff_to_nothing();
                        println!("\n{diff}");
                    }
                    (Some(&left_value), Some(&right_value)) => {
                        let left_register = register_info.decode_value(left_value);
                        let right_register = register_info.decode_value(right_value);
                        let diff = Register::diff(&left_register, &right_register).unwrap();

                        match diff {
                            Some(diff) => {
                                println!("\n{diff}");
                            }
                            None => {
                                println!("---");
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn parse_input(i: Input) -> anyhow::Result<HashMap<String, u64>> {
    let regex = Regex::new("^([^:]+): 0x([0-9a-f]+)$")?;
    let mut hm = HashMap::new();
    for line in BufReader::new(i).lines() {
        let line = line.unwrap();
        let (_, [identifier, value]) = regex.captures(&line).map(|v| v.extract()).unwrap();
        hm.insert(
            identifier.to_owned(),
            u32::from_str_radix(value, 16).map(|v| v as _)?,
        );
    }
    Ok(hm)
}

mod cli {
    use clap::{Parser, Subcommand};
    use clio::{Input, Output};
    use regex::Regex;

    #[derive(Subcommand, Debug)]
    pub enum Mode {
        DryRun {
            registers: Regex,

            #[arg(short = 'e', long, value_parser, default_value = "-")]
            exclude: Option<Regex>,
        },
        Read {
            registers: Regex,

            #[arg(value_parser, default_value = "-")]
            output: Output,

            #[arg(short = 'e', long, value_parser, default_value = "-")]
            exclude: Option<Regex>,

            #[arg(long)]
            chip: String,

            #[arg(long)]
            probe_serial: Option<String>,
        },
        Show {
            registers: Regex,

            input: Input,

            #[arg(short = 'e', long, value_parser, default_value = "-")]
            exclude: Option<Regex>,
        },
        Compare {
            registers: Regex,

            left: Input,

            right: Input,

            #[arg(short = 'e', long, value_parser, default_value = "-")]
            exclude: Option<Regex>,
        },
    }

    impl Mode {
        pub fn registers(&self) -> Regex {
            match self {
                Mode::DryRun { registers, .. } => registers.clone(),
                Mode::Read { registers, .. } => registers.clone(),
                Mode::Show { registers, .. } => registers.clone(),
                Mode::Compare { registers, .. } => registers.clone(),
            }
        }
        pub fn exclude(&self) -> Option<Regex> {
            match self {
                Mode::DryRun { exclude, .. } => exclude.clone(),
                Mode::Read { exclude, .. } => exclude.clone(),
                Mode::Show { exclude, .. } => exclude.clone(),
                Mode::Compare { exclude, .. } => exclude.clone(),
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
