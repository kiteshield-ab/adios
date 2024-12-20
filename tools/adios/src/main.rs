use std::io::{BufReader, Read};

use adi::VmStateStep;
use clap::Parser;
use cli::Args;
use regdoctor::{Database, Register};

mod adi;
mod cli;

fn main() {
    env_logger::Builder::from_env(
        // regdoctor is unhappy about colliding entries in the SVD
        // I'm not sure if turning these into INFOs in the code makes sense.
        // For now, I'm making it less noisy from the probe-compare itself
        env_logger::Env::default().default_filter_or("regdoctor=warn,warn"),
    )
    .init();

    let mut args = Args::parse();

    let mut mem_ap_db = Database::new();
    for mut svd_file in args.svd {
        let mut svd_as_string = String::new();
        svd_file.read_to_string(&mut svd_as_string).unwrap();
        let device = svd_parser::parse(&svd_as_string).unwrap();
        mem_ap_db.extend_with_svd(device);
    }
    let adi_db = regdoctor_adios_ext::Database::new();

    let adi_commands = match args.mode {
        cli::Mode::CmsisDapWsPdml => {
            let pdml_file = BufReader::new(args.input);
            adios_from_cmsis_dap_ws_pdml::generate_vm_input(pdml_file)
        }
        cli::Mode::SigrokSwd => {
            let mut swd_string = String::new();
            args.input.read_to_string(&mut swd_string).unwrap();
            adios_from_sigrok_swd::generate_vm_commands(&swd_string)
                .unwrap()
                .into_iter()
                .map(|v| v.into())
                .collect()
        }
    };

    let mut vm = adi::Vm::new();
    while let Some(step) = vm.step_forward(&adi_commands) {
        let VmStateStep {
            operations,
            previous: (previous_state, _step_i),
            current: (current_state, _),
        } = step;
        for operation in operations {
            match operation {
                adi::Operation::MemAp {
                    ts,
                    apsel,
                    rw,
                    address,
                    value,
                } if args.raw_mem_ap => {
                    match ts {
                        Some(ts) if args.ts => {
                            print!("{}-{}:", ts.start, ts.end);
                        }
                        _ => {}
                    }
                    let rw_arrow = rw.arrow();
                    let value = value.as_() as u64;
                    print!("{rw}:AP[{apsel}]:{address:#010x} {rw_arrow} {value:#010x}");
                    match mem_ap_db.get_register(address as _) {
                        Some(register_info) => {
                            println!(" ({})", register_info.identifier())
                        }
                        None => {
                            println!()
                        }
                    }
                }
                adi::Operation::DpRegisterAccess {
                    ts,
                    rw,
                    name,
                    value,
                } if args.raw_dp => {
                    match ts {
                        Some(ts) if args.ts => {
                            print!("{}-{}:", ts.start, ts.end);
                        }
                        _ => {}
                    }
                    let rw_arrow = rw.arrow();
                    println!("{rw}:DP.{name} {rw_arrow} {value:#010x}");
                }
                adi::Operation::ApRegisterAccess {
                    ts,
                    apsel,
                    rw,
                    name,
                    value,
                } if args.raw_ap => {
                    match ts {
                        Some(ts) if args.ts => {
                            print!("{}-{}:", ts.start, ts.end);
                        }
                        _ => {}
                    }
                    let rw_arrow = rw.arrow();
                    println!("{rw}:AP[{apsel}].{name} {rw_arrow} {value:#010x}");
                }
                adi::Operation::Landmark { message: metadata } => {
                    println!("!:{metadata}");
                }
                _ => {}
            }
        }

        if !args.mem_diffs {
            continue;
        }

        for (apsel, ap) in current_state.aps.iter().enumerate() {
            let csw_type = ap.idr.map_or_else(
                || regdoctor_adios_ext::CswType::Generic,
                |v| v.type_().csw_type(),
            );
            match (previous_state.aps[apsel].csw, ap.csw) {
                (None, Some(new_csw)) => {
                    let new_value = u32::from(new_csw);
                    let register_info = adi_db.ap_csw(csw_type);
                    let value = register_info.decode_value(new_value as _);
                    let diff_from_nothing = value.diff_from_nothing();
                    println!("{} / AP[{apsel}]", register_info.identifier());
                    println!("{diff_from_nothing}");
                }
                (Some(old_csw), Some(new_csw)) => {
                    let old_value = u32::from(old_csw);
                    let new_value = u32::from(new_csw);
                    let register_info = adi_db.ap_csw(csw_type);
                    let old = register_info.decode_value(old_value as _);
                    let new = register_info.decode_value(new_value as _);
                    if let Some(diff) = Register::diff(&old, &new)
                        .expect("Different registers on the same address?")
                    {
                        println!("{} / AP[{apsel}]", register_info.identifier());
                        println!("{diff}");
                    }
                }
                (Some(_), None) => unreachable!("CSW should not ever disappear"),
                (None, None) => {}
            }
            for (&address, &new_value) in ap.memory.iter() {
                match previous_state.aps[apsel].memory.get(&address) {
                    Some(&old_value) => {
                        if old_value != new_value {
                            println!("U:AP[{apsel}]:{address:#010x} : {old_value:#010x} → {new_value:#010x}");
                            match mem_ap_db.get_register(address as _) {
                                Some(register_info) => {
                                    let old = register_info.decode_value(old_value as _);
                                    let new = register_info.decode_value(new_value as _);
                                    let Some(diff) = Register::diff(&old, &new)
                                        .expect("Different registers on the same address?")
                                    else {
                                        continue;
                                    };
                                    println!("{}", register_info.identifier());
                                    println!("{diff}");
                                }
                                None => {}
                            }
                        }
                    }
                    None => {
                        println!("N:AP[{apsel}]:{address:#010x} : 0x???????? → {new_value:#010x}");
                        match mem_ap_db.get_register(address as _) {
                            Some(register_info) => {
                                let value = register_info.decode_value(new_value as _);
                                let diff_from_nothing = value.diff_from_nothing();
                                println!("{}", register_info.identifier());
                                println!("{diff_from_nothing}");
                            }
                            None => {}
                        };
                    }
                }
            }
        }
    }
}
