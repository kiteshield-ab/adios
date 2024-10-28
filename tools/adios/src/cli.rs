use clap::{Parser, ValueEnum};
use clio::Input;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Mode {
    /// PDML file generated with wireshark/tshark
    ///
    /// - It must exclusively contain CMSIS-DAP traffic between the host and the probe.
    /// - It must contain decoded CMSIS-DAP communication via
    /// https://github.com/glaeqen/cmsis-dap-v2-dissector
    CmsisDapWsPdml,
    /// TXT file generated via sigrok-cli
    ///
    /// - It must contain a decoded list of SWD commands from the sigrok's SWD decoder
    // Example:
    // ```
    // 67488474-67506918 swd-1: LINERESET
    // 67506918-67507035 swd-1: JTAG->SWD
    // 67518415-67525227 swd-1: LINERESET
    // 67530873-67530911 swd-1: IDCODE
    // 67530922-67530932 swd-1: OK
    // 67530942-67531169 swd-1: 0x5ba02477
    // 69672366-69672404 swd-1: IDCODE
    // 69672415-69672425 swd-1: OK
    // 69672435-69672662 swd-1: 0x5ba02477
    // ```
    SigrokSwd,
}

/// ARM ADIv5 replaying tool
#[derive(Parser, Debug)]
pub struct Args {
    /// List of SVD files used for register decoding
    #[arg(short = 's', long, value_parser)]
    pub svd: Vec<Input>,

    /// An input file which content is interpreted depending on the chosen `--mode`
    #[arg(short = 'i', long, value_parser)]
    pub input: Input,

    #[arg(long, value_enum)]
    pub mode: Mode,

    /// Show memory diffs for every MEM-AP target between each VM step
    #[arg(short = 'm', long, default_value_t = false)]
    pub mem_diffs: bool,

    /// Show raw MEM-AP accesses
    #[arg(short = 'M', long, default_value_t = false)]
    pub raw_mem_ap: bool,

    /// Show raw DP accesses
    #[arg(long = "dp", default_value_t = false)]
    pub raw_dp: bool,

    /// Show raw AP accesses
    #[arg(long = "ap", default_value_t = false)]
    pub raw_ap: bool,

    /// Enable timestamps (if available (SWD - yes, PDML - no))
    #[arg(long = "ts", default_value_t = false)]
    pub ts: bool,
}
