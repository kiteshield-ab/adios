use ap::Idr;
use bilge::prelude::*;
use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    rc::Rc,
};

use adios_common::{Input, Timestamp};

pub struct Vm {
    command_cursor: usize,
    state: VmState,
}

#[derive(Clone)]
pub struct VmState {
    pub dp: Dp,
    pub aps: [Ap; 256],
}

#[derive(Default, Copy, Clone, Debug)]
pub struct Dp {
    pub select: dp::Select,
}

pub mod dp {
    use super::*;

    #[bitsize(32)]
    #[derive(Default, FromBits, Copy, Clone, DebugBits)]
    pub struct Select {
        pub dpbanksel: u4,
        pub apbanksel: u4,
        pub reserved: u16,
        pub apsel: u8,
    }
}

#[derive(PartialEq, Eq, Default, Clone, Debug)]
pub struct Ap {
    pub memory: HashMap<u32, u32>,
    pub tar: Option<u32>,
    pub csw: Option<ap::Csw>,
    pub idr: Option<ap::Idr>,
}

pub mod ap {
    use regdoctor_adios_ext::CswType;

    use super::*;

    #[bitsize(32)]
    #[derive(Default, TryFromBits, Copy, Clone, DebugBits, PartialEq, Eq)]
    pub struct Csw {
        pub size: CswSize,
        pub reserved: u1,
        pub addr_inc: CswAddrInc,
        pub device_en: bool,
        pub transfer_in_progress: bool,
        pub reserved: u15,
        pub secure_debug: bool,
        pub protection: u7, // Impl defined what this does, would be cool to know
        pub dbg_sw_enable: bool,
    }

    impl Csw {
        pub fn overwrite_rw_fields(&mut self, other: &Self) {
            self.set_size(other.size()); // Unclear when supported but IMXRT118x definitely supports this.
            self.set_addr_inc(other.addr_inc());
            self.set_transfer_in_progress(other.transfer_in_progress());
            self.set_protection(other.protection()); // Unclear when supported but IMXRT118x definitely supports this.
        }
    }

    #[bitsize(2)]
    #[derive(Default, TryFromBits, Copy, Clone, Debug, PartialEq, Eq)]
    pub enum CswAddrInc {
        #[default]
        Disabled = 0b00,
        Single = 0b01,
        // Packed increment is not supported
        // Packed = 0b10,
    }

    #[bitsize(3)]
    #[derive(Default, TryFromBits, Copy, Clone, Debug, PartialEq, Eq)]
    pub enum CswSize {
        #[default]
        Byte = 0b000,
        Halfword = 0b001,
        Word = 0b010,
        // MEM-AP Large Data Extension is not supported
        // Doubleword = 0b011,
        // Bits128 = 0b100,
        // Bits256 = 0b101,
    }

    #[bitsize(32)]
    #[derive(Default, FromBits, Copy, Clone, DebugBits, PartialEq, Eq)]
    pub struct Idr {
        pub type_: IdrType,
        pub variant: u4,
        pub res0: u5,
        pub class: IdrClass,
        pub designed: u11,
        pub revision: u4,
    }

    #[bitsize(4)]
    #[repr(u8)]
    #[derive(FromBits, Copy, Clone, Debug, PartialEq, Eq)]
    pub enum IdrType {
        JtagConnectionOrComAp = 0x0,
        AmbaAhb3Bus = 0x1,
        AmbaApb2OrApb3Bus = 0x2,
        AmbaAxi3OrAxi4BusWithOptionalAceLiteSupport = 0x4,
        AmbaAhb5Bus = 0x5,
        AmbaApb4AndApb5Bus = 0x6,
        AmbaAxi5Bus = 0x7,
        AmbaAhb5WithEnhancedHprot = 0x8,
        #[fallback]
        Reserved(u4),
    }

    impl IdrType {
        pub fn is_unknown(&self) -> bool {
            *self == Self::default()
        }

        pub fn csw_type(&self) -> CswType {
            match self {
                Self::AmbaAhb3Bus => CswType::AmbaAhb3,
                _ => CswType::Generic,
            }
        }
    }

    impl Default for IdrType {
        fn default() -> Self {
            Self::Reserved(u4::new(0b0))
        }
    }

    #[bitsize(4)]
    #[derive(Default, FromBits, Copy, Clone, Debug, PartialEq, Eq)]
    pub enum IdrClass {
        #[default]
        #[fallback]
        Undefined = 0b0000,
        ComAccessPort = 0b0001,
        MemoryAccessPort = 0b1000,
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum RoW {
    R,
    W,
}

impl RoW {
    pub fn arrow(&self) -> &'static str {
        match self {
            RoW::R => "→",
            RoW::W => "←",
        }
    }
}

impl Display for RoW {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RoW::R => f.write_str("R"),
            RoW::W => f.write_str("W"),
        }
    }
}

impl Vm {
    pub fn new() -> Self {
        Self {
            command_cursor: 0,
            state: Default::default(),
        }
    }

    pub fn step_forward(&mut self, commands: &Vec<Input>) -> Option<VmStateStep> {
        let command = commands.get(self.command_cursor)?;
        let previous_state = self.state.clone();
        let operations = self.state.step(command.clone());
        let current_state = self.state.clone();
        self.command_cursor += 1;
        Some(VmStateStep {
            operations,
            previous: (previous_state, self.command_cursor - 1),
            current: (current_state, self.command_cursor),
        })
    }

    #[allow(unused)]
    pub fn step_back(&mut self, commands: &Vec<Input>) -> Option<VmStateStep> {
        let previous_state = self.state.clone();
        let command_cursor = self.command_cursor.checked_sub(1)?;
        self.state.reset();
        for command in commands.iter().take(command_cursor) {
            let _ = self.state.step(command.clone());
        }
        self.command_cursor = command_cursor;
        let current_state = self.state.clone();
        Some(VmStateStep {
            operations: Vec::new(),
            previous: (previous_state, self.command_cursor + 1),
            current: (current_state, self.command_cursor),
        })
    }
}

impl VmState {
    fn reset(&mut self) {
        *self = Default::default();
    }

    fn step(&mut self, cmd: Input) -> Vec<Operation> {
        let mut operations = Vec::new();
        let cmd = match cmd {
            Input::Landmark { metadata } => {
                operations.push(Operation::Landmark { metadata });
                return operations;
            }
            Input::Command(cmd) => cmd,
        };
        let ts = cmd.ts;
        let rw = if cmd.rnw { RoW::R } else { RoW::W };
        let a = cmd.a.value() << 2;
        if !cmd.apndp {
            match (self.dp.select.dpbanksel().value(), a, rw) {
                (_, 0x0, RoW::R) => {
                    log::debug!("DP.DPIDR: {:#0x}", cmd.data);
                    operations.push(Operation::DpRegisterAccess {
                        ts,
                        rw,
                        value: cmd.data,
                        name: "DPIDR",
                    });
                }
                (_, 0x0, RoW::W) => {
                    log::debug!("DP.ABORT: {:#0x}", cmd.data);
                    operations.push(Operation::DpRegisterAccess {
                        ts,
                        rw,
                        value: cmd.data,
                        name: "ABORT",
                    });
                }
                (0x0, 0x4, rw) => {
                    // TODO: Add more verbose information, verify
                    // that NXP is not using some weird MEM-AP features
                    // that has to be handled, like transaction counters?
                    log::debug!("DP.CTRL/STAT: {}:{:#0x}", rw, cmd.data);
                    operations.push(Operation::DpRegisterAccess {
                        ts,
                        rw,
                        value: cmd.data,
                        name: "CTRL",
                    });
                }
                (0x1, 0x4, rw) => {
                    log::debug!("DP.DLCR: {}:{:#0x}", rw, cmd.data);
                    operations.push(Operation::DpRegisterAccess {
                        ts,
                        rw,
                        value: cmd.data,
                        name: "DLCR",
                    });
                }
                (0x2, 0x4, RoW::R) => {
                    log::debug!("DP.TARGETID: R:{:#0x}", cmd.data);
                    operations.push(Operation::DpRegisterAccess {
                        ts,
                        rw,
                        value: cmd.data,
                        name: "TARGETID",
                    });
                }
                (0x3, 0x4, RoW::R) => {
                    log::debug!("DP.DLPIDR: R:{:#0x}", cmd.data);
                    operations.push(Operation::DpRegisterAccess {
                        ts,
                        rw,
                        value: cmd.data,
                        name: "DLPIDR",
                    });
                }
                (0x4, 0x4, RoW::R) => {
                    log::debug!("DP.EVENTSTAT: R:{:#0x}", cmd.data);
                    operations.push(Operation::DpRegisterAccess {
                        ts,
                        rw,
                        value: cmd.data,
                        name: "EVENTSTAT",
                    });
                }
                (_, 0x8, RoW::R) => {
                    log::debug!("DP.RESEND: R:{:#0x}", cmd.data);
                    operations.push(Operation::DpRegisterAccess {
                        ts,
                        rw,
                        value: cmd.data,
                        name: "RESEND",
                    });
                }
                (_, 0x8, RoW::W) => {
                    let new_select = dp::Select::from(cmd.data);
                    log::debug!("DP.SELECT: {:#0x?} -> {:#0x?}", self.dp.select, new_select);
                    operations.push(Operation::DpRegisterAccess {
                        ts,
                        rw,
                        value: cmd.data,
                        name: "SELECT",
                    });
                    self.dp.select = new_select;
                }
                (_, 0xc, RoW::R) => {
                    log::debug!("DP.RDBUFF: {:#0x}", cmd.data);
                    operations.push(Operation::DpRegisterAccess {
                        ts,
                        rw,
                        value: cmd.data,
                        name: "RDBUFF",
                    });
                }
                (_, 0xc, RoW::W) => {
                    log::debug!("DP.TARGETSEL: W:{:#0x}", cmd.data);
                    operations.push(Operation::DpRegisterAccess {
                        ts,
                        rw,
                        value: cmd.data,
                        name: "TARGETSEL",
                    });
                }
                _ => {
                    log::error!("Unexpected DP cmd: {:0x?}", cmd)
                }
            }
        } else {
            let apsel = self.dp.select.apsel();
            let ap_addr = (self.dp.select.apbanksel().value() << 4) | a;
            match (ap_addr, rw) {
                (0x0, rw) => {
                    let new_csw = ap::Csw::try_from(cmd.data).unwrap();
                    // TODO: This log is a little bit confusing as readonly fields on write should be ignored
                    // Keep it?
                    log::debug!("AP[{apsel}].CSW: {}:{:#0x?}", rw, new_csw);
                    operations.push(Operation::ApRegisterAccess {
                        ts,
                        rw,
                        name: "CSW",
                        value: cmd.data,
                        apsel,
                    });
                    let csw = &mut self.current_ap_mut().csw;
                    match rw {
                        RoW::R => *csw = Some(new_csw),
                        RoW::W => match csw {
                            Some(csw) => csw.overwrite_rw_fields(&new_csw),
                            None => *csw = Some(new_csw),
                        },
                    }
                }
                (0x4, rw) => {
                    log::debug!("AP[{apsel}].TAR: {}:{:#0x}", rw, cmd.data);
                    operations.push(Operation::ApRegisterAccess {
                        ts,
                        rw,
                        name: "TAR",
                        value: cmd.data,
                        apsel,
                    });
                    if rw == RoW::W {
                        self.current_ap_mut().tar = Some(cmd.data);
                    } else {
                        assert_eq!(self.current_ap().tar, Some(cmd.data));
                    }
                }
                (0xc, rw) => {
                    // TODO: Configurability of what needs to be printed out has to be improved
                    log::debug!("AP[{apsel}].DRW: {}:{:#0x}", rw, cmd.data);
                    operations.push(Operation::ApRegisterAccess {
                        ts,
                        rw,
                        name: "DRW",
                        value: cmd.data,
                        apsel,
                    });
                    // Unwrap: If TAR is not set, we have no clue what address we are accessing.
                    let addr = self.current_ap().tar.unwrap();
                    self.drw_access(&mut operations, ts, rw, addr, cmd.data);
                }
                (0x10, rw) => {
                    log::debug!("AP[{apsel}].BD0: {}:{:#0x}", rw, cmd.data);
                    operations.push(Operation::ApRegisterAccess {
                        ts,
                        rw,
                        name: "BD0",
                        value: cmd.data,
                        apsel,
                    });
                    // Memory addressing for BDx C.2.6.2, IHI0031G
                    // Unwrap: If TAR is not set, we have no clue what address we are accessing.
                    let addr = self.current_ap().tar.unwrap() & 0xFFFFFFF0;
                    self.bd_access(&mut operations, ts, rw, addr, cmd.data);
                }
                (0x14, rw) => {
                    log::debug!("AP[{apsel}].BD1: {}:{:#0x}", rw, cmd.data);
                    operations.push(Operation::ApRegisterAccess {
                        ts,
                        rw,
                        name: "BD1",
                        value: cmd.data,
                        apsel,
                    });
                    // Memory addressing for BDx C.2.6.2, IHI0031G
                    // Unwrap: If TAR is not set, we have no clue what address we are accessing.
                    let addr = self.current_ap().tar.unwrap() & 0xFFFFFFF0 | 0x4;
                    self.bd_access(&mut operations, ts, rw, addr, cmd.data);
                }
                (0x18, rw) => {
                    log::debug!("AP[{apsel}].BD2: {}:{:#0x}", rw, cmd.data);
                    operations.push(Operation::ApRegisterAccess {
                        ts,
                        rw,
                        name: "BD2",
                        value: cmd.data,
                        apsel,
                    });
                    // Memory addressing for BDx C.2.6.2, IHI0031G
                    // Unwrap: If TAR is not set, we have no clue what address we are accessing.
                    let addr = self.current_ap().tar.unwrap() & 0xFFFFFFF0 | 0x8;
                    self.bd_access(&mut operations, ts, rw, addr, cmd.data);
                }
                (0x1c, rw) => {
                    log::debug!("AP[{apsel}].BD3: {}:{:#0x}", rw, cmd.data);
                    operations.push(Operation::ApRegisterAccess {
                        ts,
                        rw,
                        name: "BD3",
                        value: cmd.data,
                        apsel,
                    });
                    // Memory addressing for BDx C.2.6.2, IHI0031G
                    // Unwrap: If TAR is not set, we have no clue what address we are accessing.
                    let addr = self.current_ap().tar.unwrap() & 0xFFFFFFF0 | 0xc;
                    self.bd_access(&mut operations, ts, rw, addr, cmd.data);
                }
                (0xf4, rw) => {
                    log::debug!("AP[{apsel}].CFG: {}:{:#0x}", rw, cmd.data);
                    operations.push(Operation::ApRegisterAccess {
                        ts,
                        rw,
                        name: "CFG",
                        value: cmd.data,
                        apsel,
                    });
                }
                (0xf8, rw) => {
                    log::debug!("AP[{apsel}].BASE: {}:{:#0x}", rw, cmd.data);
                    operations.push(Operation::ApRegisterAccess {
                        ts,
                        rw,
                        name: "BASE",
                        value: cmd.data,
                        apsel,
                    });
                }
                (0xfc, rw) => {
                    log::debug!("AP[{apsel}].IDR: {}:{:#0x}", rw, cmd.data);
                    self.current_ap_mut().idr = Some(Idr::from(cmd.data));
                    operations.push(Operation::ApRegisterAccess {
                        ts,
                        rw,
                        name: "IDR",
                        value: cmd.data,
                        apsel,
                    });
                }
                _ => {
                    log::error!(
                        "Unexpected AP cmd: {:0x?} (DP.SELECT: {:0x?})",
                        cmd,
                        self.dp.select
                    )
                }
            }
        }
        operations
    }

    fn current_ap(&self) -> &Ap {
        &self.aps[self.dp.select.apsel() as usize]
    }

    fn current_ap_mut(&mut self) -> &mut Ap {
        &mut self.aps[self.dp.select.apsel() as usize]
    }

    // Apply address increment if enabled according to CSW configuration
    fn drw_access(
        &mut self,
        operations: &mut Vec<Operation>,
        ts: Option<Timestamp>,
        rw: RoW,
        address: u32,
        value: u32,
    ) {
        // Unwrap: If CSW is not known, VM does not know data access details
        let csw = self.current_ap().csw.unwrap();
        let tar_two_lsbs = (address as u8) & 0b11;
        log::debug!("Access size: {:?}", csw.size());
        log::debug!("Address incrementing: {:?}", csw.addr_inc());

        // Byte lanes C.2.2.6, IHI0031G
        let mem_value = self
            .current_ap_mut()
            .memory
            .entry(address & 0xFFFFFFFC)
            .or_insert(0x0);
        // Does not matter if read or write, this is a simulator after all
        let value = match (tar_two_lsbs, csw.size()) {
            // TODO: unittest? `value` might be incorrectly interpreted here
            (0b00, ap::CswSize::Word) => {
                *mem_value = value;
                value
            }
            (0b00, ap::CswSize::Halfword) => {
                let value = value & 0x0000FFFF;
                *mem_value = (*mem_value & 0xFFFF0000) | value;
                value
            }
            (0b10, ap::CswSize::Halfword) => {
                let value = value & 0xFFFF0000;
                *mem_value = (*mem_value & 0x0000FFFF) | value;
                value >> 16
            }
            (0b00, ap::CswSize::Byte) => {
                let value = value & 0x000000FF;
                *mem_value = (*mem_value & 0xFFFFFF00) | value;
                value
            }
            (0b01, ap::CswSize::Byte) => {
                let value = value & 0x0000FF00;
                *mem_value = (*mem_value & 0xFFFF00FF) | value;
                value >> 8
            }
            (0b10, ap::CswSize::Byte) => {
                let value = value & 0x00FF0000;
                *mem_value = (*mem_value & 0xFF00FFFF) | value;
                value >> 16
            }
            (0b11, ap::CswSize::Byte) => {
                let value = value & 0xFF000000;
                *mem_value = (*mem_value & 0x00FFFFFF) | value;
                value >> 24
            }
            (_lsbs, _size) => {
                log::error!("Invalid or impl defined DRW byte lanes usage?");
                return;
            }
        };

        let rw_arrow = rw.arrow();
        let mem_ap_value = match csw.size() {
            ap::CswSize::Word => {
                log::info!("{rw}:{address:#010x} {rw_arrow} {:#010x}", value);
                MemApValue::Word(value)
            }
            ap::CswSize::Halfword => {
                let value = value as u16;
                log::info!("{rw}:{address:#010x} {rw_arrow} {:#06x}", value);
                MemApValue::Halfword(value)
            }
            ap::CswSize::Byte => {
                let value = value as u8;
                log::info!("{rw}:{address:#010x} {rw_arrow} {:#04x}", value);
                MemApValue::Byte(value)
            }
        };
        operations.push(Operation::MemAp {
            ts,
            apsel: self.dp.select.apsel(),
            rw,
            address,
            value: mem_ap_value,
        });

        // Unwrap: If we got here, we must have accessed TAR so it exists for sure
        let tar = &mut self.current_ap_mut().tar.unwrap();
        match (csw.addr_inc(), csw.size()) {
            (ap::CswAddrInc::Single, ap::CswSize::Word) => *tar += 4,
            (ap::CswAddrInc::Single, ap::CswSize::Halfword) => *tar += 2,
            (ap::CswAddrInc::Single, ap::CswSize::Byte) => *tar += 1,
            _ => {}
        }
    }

    fn bd_access(
        &mut self,
        operations: &mut Vec<Operation>,
        ts: Option<Timestamp>,
        rw: RoW,
        address: u32,
        value: u32,
    ) {
        let rw_arrow = rw.arrow();
        self.current_ap_mut().memory.insert(address, value);
        log::info!("{rw}:{address:#010x} {rw_arrow} {value:#010x}");
        operations.push(Operation::MemAp {
            ts,
            apsel: self.dp.select.apsel(),
            rw,
            address,
            value: MemApValue::Word(value),
        });
    }
}

impl Default for VmState {
    fn default() -> Self {
        Self {
            dp: Dp::default(),
            aps: core::array::from_fn(|_| Ap::default()),
        }
    }
}

pub struct VmStateStep {
    /// Empty on [`Vm::step_back`]
    pub operations: Vec<Operation>,
    pub previous: (VmState, usize),
    pub current: (VmState, usize),
}

pub enum Operation {
    Landmark {
        metadata: Rc<dyn Display>,
    },
    DpRegisterAccess {
        ts: Option<Timestamp>,
        rw: RoW,
        name: &'static str,
        value: u32,
    },
    ApRegisterAccess {
        ts: Option<Timestamp>,
        apsel: u8,
        rw: RoW,
        name: &'static str,
        value: u32,
    },
    MemAp {
        ts: Option<Timestamp>,
        apsel: u8,
        rw: RoW,
        address: u32,
        value: MemApValue,
    },
}

#[derive(Copy, Clone)]
pub enum MemApValue {
    Word(u32),
    Halfword(u16),
    Byte(u8),
}

impl MemApValue {
    pub fn as_(&self) -> u32 {
        match *self {
            MemApValue::Word(v) => v,
            MemApValue::Halfword(v) => v as _,
            MemApValue::Byte(v) => v as _,
        }
    }
}
