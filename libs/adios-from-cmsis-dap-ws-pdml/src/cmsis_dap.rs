#![allow(unused)]

pub mod from_pdml;

#[derive(Clone, Debug)]
pub struct Frame {
    pub number: usize,
    pub time_epoch: String,
    pub content: Content,
}

#[derive(Clone, Debug)]
pub enum Content {
    CmsisDapRequest {
        content: Request,
        corresponding_response: usize,
    },
    CmsisDapResponse {
        content: Response,
        corresponding_request: usize,
    },
}

#[derive(Clone, Debug)]
pub enum Response {
    // 0
    DapInfo,
    // 1
    DapHostStatus,
    // 2
    DapConnect(response::DapConnect),
    // 3
    DapDisconnect(response::DapDisconnect),
    // 4
    DapTransferConfigure(response::DapTransferConfigure),
    // 5
    DapTransfer(response::DapTransfer),
    // 6
    DapTransferBlock(response::DapTransferBlock),
    // 7
    DapTransferAbort,
    // 8
    DapWriteAbort(response::DapWriteAbort),
    // 9
    DapDelay,
    // 10
    DapResetTarget,
    // 16
    DapSwjPins,
    // 17
    DapSwjClock(response::DapSwjClock),
    // 18
    DapSwjSequence(response::DapSwjSequence),
    // 19
    DapSwdConfigure(response::DapSwdConfigure),
    // 20
    DapJtagSequence,
    // 21
    DapJtagConfigure,
    // 22
    DapJtagIdcode,
    // 23
    DapSwoTransport,
    // 24
    DapSwoMode,
    // 25
    DapSwoBaudrate,
    // 26
    DapSwoControl,
    // 27
    DapSwoStatus,
    // 28
    DapSwoData,
    // 29
    DapSwdSequence,
    // 30
    DapSwdExtendedStatus,
    // 31
    DapUartTransport,
    // 32
    DapUartConfigure,
    // 33
    DapUartTransfer,
    // 34
    DapUartControl,
    // 35
    DapUartStatus,
    // 126
    DapQueueCommands,
    // 127
    DapExecuteCommands,
    Unknown { header_byte: u8, raw_data: Vec<u8> },
}

#[derive(Clone, Debug)]
pub enum Request {
    // 0
    DapInfo,
    // 1
    DapHostStatus,
    // 2
    DapConnect(request::DapConnect),
    // 3
    DapDisconnect(request::DapDisconnect),
    // 4
    DapTransferConfigure(request::DapTransferConfigure),
    // 5
    DapTransfer(request::DapTransfer),
    // 6
    DapTransferBlock(request::DapTransferBlock),
    // 7
    DapTransferAbort,
    // 8
    DapWriteAbort(request::DapWriteAbort),
    // 9
    DapDelay,
    // 10
    DapResetTarget,
    // 16
    DapSwjPins,
    // 17
    DapSwjClock(request::DapSwjClock),
    // 18
    DapSwjSequence(request::DapSwjSequence),
    // 19
    DapSwdConfigure(request::DapSwdConfigure),
    // 20
    DapJtagSequence,
    // 21
    DapJtagConfigure,
    // 22
    DapJtagIdcode,
    // 23
    DapSwoTransport,
    // 24
    DapSwoMode,
    // 25
    DapSwoBaudrate,
    // 26
    DapSwoControl,
    // 27
    DapSwoStatus,
    // 28
    DapSwoData,
    // 29
    DapSwdSequence,
    // 30
    DapSwdExtendedStatus,
    // 31
    DapUartTransport,
    // 32
    DapUartConfigure,
    // 33
    DapUartTransfer,
    // 34
    DapUartControl,
    // 35
    DapUartStatus,
    // 126
    DapQueueCommands,
    // 127
    DapExecuteCommands,
    Unknown { header_byte: u8, raw_data: Vec<u8> },
}

pub mod request {
    use std::num::{NonZeroU16, NonZeroU8};

    use bilge::prelude::*;

    #[derive(Clone, Debug)]
    pub struct DapConnect {
        pub swd_port: u8,
    }

    #[derive(Clone, Debug)]
    pub struct DapDisconnect;

    #[derive(Clone, Debug)]
    pub struct DapTransferConfigure {
        pub idle_cycles: u8,
        pub wait_retry: u16,
        pub match_retry: u16,
    }

    #[derive(Clone, Debug)]
    pub struct DapTransfer {
        pub dap_index: u8,
        pub transfer_count: NonZeroU8,
        pub transfers: Vec<DapSingleTransfer>,
    }

    #[derive(Clone, Debug)]
    pub struct DapSingleTransfer {
        pub request: DapTransferRequest,
        pub data: Option<u32>,
    }

    #[bitsize(8)]
    #[derive(FromBits, Clone, DebugBits)]
    pub struct DapTransferRequest {
        pub apndp: bool,
        pub rnw: bool,
        pub a2: bool,
        pub a3: bool,
        pub value_match: bool,
        pub match_mask: bool,
        pub reserved: bool,
        pub timestamp_request: bool,
    }

    #[derive(Clone, Debug)]
    pub struct DapTransferBlock {
        pub dap_index: u8,
        pub transfer_count: NonZeroU16,
        pub request: DapTransferBlockRequest,
        pub data: Vec<u32>,
    }

    #[bitsize(8)]
    #[derive(FromBits, Clone, DebugBits)]
    pub struct DapTransferBlockRequest {
        pub apndp: bool,
        pub rnw: bool,
        pub a2: bool,
        pub a3: bool,
        pub reserved: u4,
    }

    #[derive(Clone, Debug)]
    pub struct DapWriteAbort {
        pub dap_index: u8,
        pub abort: u32,
    }

    #[derive(Clone, Debug)]
    pub struct DapSwjClock {
        pub clock: u32,
    }

    #[derive(Clone, Debug)]
    pub struct DapSwjSequence {
        pub bit_count: usize,
        pub bit_data: Vec<u8>,
    }

    #[bitsize(3)]
    #[derive(FromBits, Clone, DebugBits)]
    pub struct DapSwdConfigure {
        pub turnaround_clock_period: u2,
        pub data_phase: bool,
    }
}

pub mod response {
    use bilge::prelude::*;

    #[derive(Clone, Debug)]
    pub enum DapResponseStatus {
        Ok = 0x00,
        Err = 0xFF,
    }

    #[derive(Clone, Debug)]
    pub struct DapConnect {
        pub swd_port: u8,
    }

    #[derive(Clone, Debug)]
    pub struct DapDisconnect {
        pub status: DapResponseStatus,
    }

    #[derive(Clone, Debug)]
    pub struct DapTransferConfigure {
        pub status: DapResponseStatus,
    }

    #[derive(Clone, Debug)]
    pub struct DapTransfer {
        pub transfer_count: u8,
        pub response: DapTransferResponse,
        pub data: Vec<u32>,
    }

    #[bitsize(8)]
    #[derive(TryFromBits, Clone, DebugBits)]
    pub struct DapTransferResponse {
        pub ack: DapTransferResponseAck,
        pub protocol_error: bool,
        pub value_mismatch: bool,
        pub reserved: u3,
    }

    #[bitsize(3)]
    #[derive(TryFromBits, Clone, Debug, PartialEq)]
    pub enum DapTransferResponseAck {
        // Ok for SWD, OK or FAULT for JTAG
        // Weird but ok.
        // https://arm-software.github.io/CMSIS-DAP/latest/group__DAP__Transfer.html
        Ok = 1,
        Wait = 2,
        Fault = 4,
        NoAck = 7,
    }

    #[derive(Clone, Debug)]
    pub struct DapTransferBlock {
        pub transfer_count: u8,
        pub response: DapTransferBlockResponse,
        pub data: Vec<u32>,
    }

    #[bitsize(8)]
    #[derive(TryFromBits, Clone, DebugBits)]
    pub struct DapTransferBlockResponse {
        pub ack: DapTransferResponseAck,
        pub protocol_error: bool,
        pub reserved: u4,
    }

    #[derive(Clone, Debug)]
    pub struct DapWriteAbort {
        pub status: DapResponseStatus,
    }

    #[derive(Clone, Debug)]
    pub struct DapSwjClock {
        pub status: DapResponseStatus,
    }

    #[derive(Clone, Debug)]
    pub struct DapSwjSequence {
        pub status: DapResponseStatus,
    }

    #[derive(Clone, Debug)]
    pub struct DapSwdConfigure {
        pub status: DapResponseStatus,
    }
}
