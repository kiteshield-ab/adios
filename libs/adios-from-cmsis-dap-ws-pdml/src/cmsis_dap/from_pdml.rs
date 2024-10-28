use bilge::prelude::*;
use std::num::{NonZeroU16, NonZeroU8};

use crate::pdml;

use super::{request, response, Content, Frame, Request, Response};

impl Frame {
    pub fn from_pdml_packet(packet: &pdml::Packet) -> Option<Self> {
        let mut number: Option<usize> = None;
        let mut time_epoch: Option<String> = None;
        let mut cmsis_dap: Option<pdml::Proto> = None;
        for proto in packet.proto.iter() {
            if proto.name == "frame" {
                for field in proto.field.iter() {
                    if field.name == "frame.number" {
                        log::trace!("frame.number: {}", field.show);
                        assert!(
                            number.replace(field.show.parse().unwrap()).is_none(),
                            "Frame number shows up more than once"
                        )
                    }
                    if field.name == "frame.time_epoch" {
                        log::trace!("frame.time_epoch: {}", field.show);
                        assert!(
                            time_epoch.replace(field.show.clone()).is_none(),
                            "Time epoch shows up more than once"
                        )
                    }
                }
            }
            if proto.name == "usbdap" {
                log::trace!("proto.usbdap found");
                assert!(
                    cmsis_dap.replace(proto.clone()).is_none(),
                    "CMSIS-DAP protocol shows up more than once"
                )
            }
        }

        let number = number.unwrap();
        let time_epoch = time_epoch.unwrap();

        if cmsis_dap.is_none() {
            log::debug!("Frame {} is not a CMSIS-DAP frame", number);
        }

        let content = super::Content::from_pdml_proto(cmsis_dap?);

        Some(Self {
            number,
            time_epoch,
            content,
        })
    }
}

impl Content {
    fn from_pdml_proto(proto: pdml::Proto) -> Self {
        assert_eq!(proto.name, "usbdap", "Not CMSIS-DAP protocol");
        enum CommandType {
            Request { corresponding_response: usize },
            Response { corresponding_request: usize },
        }
        let mut command_type: Option<CommandType> = None;
        for field in proto.field.iter() {
            if field.name == "cmsis_dap.response" {
                // If a protocol contains a response field, it is a request
                log::trace!("cmsis_dap.response for request: {}", field.show);
                assert!(
                    command_type
                        .replace(CommandType::Request {
                            corresponding_response: field.show.parse().unwrap()
                        })
                        .is_none(),
                    "Field shows up more than once"
                )
            }
            if field.name == "cmsis_dap.request" {
                log::trace!("cmsis_dap.request for response: {}", field.show);
                // If a protocol contains a request field, it is a response
                assert!(
                    command_type
                        .replace(CommandType::Response {
                            corresponding_request: field.show.parse().unwrap()
                        })
                        .is_none(),
                    "Field shows up more than once"
                )
            }
        }
        let command_type = command_type.unwrap();
        match command_type {
            CommandType::Request {
                corresponding_response,
            } => Content::CmsisDapRequest {
                content: Request::from_pdml_proto(proto),
                corresponding_response,
            },
            CommandType::Response {
                corresponding_request,
            } => Content::CmsisDapResponse {
                content: Response::from_pdml_proto(proto),
                corresponding_request,
            },
        }
    }
}

impl Response {
    pub fn from_pdml_proto(proto: pdml::Proto) -> Self {
        // TODO: Export to some common function
        let mut command_header_byte: Option<u8> = None;
        for field in proto.field.iter() {
            if field.name == "cmsis_dap.command" {
                log::trace!("cmsis_dap.command: {}", field.show);
                let value = u8::from_str_radix(field.show.trim_start_matches("0x"), 16).unwrap();
                assert!(
                    command_header_byte.replace(value).is_none(),
                    "Field shows up more than once"
                );
                break;
            }
        }
        let command_header_byte = command_header_byte.unwrap();
        match command_header_byte {
            0x02 => Self::DapConnect(response::DapConnect::from_pdml_proto(proto)),
            0x03 => Self::DapDisconnect(response::DapDisconnect::from_pdml_proto(proto)),
            0x04 => {
                Self::DapTransferConfigure(response::DapTransferConfigure::from_pdml_proto(proto))
            }
            0x05 => Self::DapTransfer(response::DapTransfer::from_pdml_proto(proto)),
            0x06 => Self::DapTransferBlock(response::DapTransferBlock::from_pdml_proto(proto)),
            0x08 => Self::DapWriteAbort(response::DapWriteAbort::from_pdml_proto(proto)),
            0x11 => Self::DapSwjClock(response::DapSwjClock::from_pdml_proto(proto)),
            0x12 => Self::DapSwjSequence(response::DapSwjSequence::from_pdml_proto(proto)),
            0x13 => Self::DapSwdConfigure(response::DapSwdConfigure::from_pdml_proto(proto)),
            header_byte => {
                log::warn!("Unknown CMSIS-DAP response? byte: {:#0X}", header_byte);
                Self::Unknown {
                    header_byte,
                    raw_data: raw_data_from_pdml_proto(proto),
                }
            }
        }
    }
}

fn raw_data_from_pdml_proto(proto: pdml::Proto) -> Vec<u8> {
    log::trace!("cmsis_dap.unknown");
    // 0 means 256
    let mut raw_data: Option<String> = None;
    for field in proto.field.iter() {
        if field.name == "cmsis_dap.unknown" {
            log::trace!("cmsis_dap.unknown: {}", field.show);
            let value = field.show.clone();
            assert!(
                raw_data.replace(value).is_none(),
                "Field shows up more than once"
            );
        }
    }

    let raw_data = raw_data.unwrap();
    raw_data
        .split(":")
        .map(|v| u8::from_str_radix(v, 16).unwrap())
        .collect()
}

impl Request {
    pub fn from_pdml_proto(proto: pdml::Proto) -> Self {
        let mut command_header_byte: Option<u8> = None;
        for field in proto.field.iter() {
            if field.name == "cmsis_dap.command" {
                log::trace!("cmsis_dap.command: {}", field.show);
                let value = u8::from_str_radix(field.show.trim_start_matches("0x"), 16).unwrap();
                assert!(
                    command_header_byte.replace(value).is_none(),
                    "Field shows up more than once"
                );
                break;
            }
        }
        let command_header_byte = command_header_byte.unwrap();
        match command_header_byte {
            0x02 => Self::DapConnect(request::DapConnect::from_pdml_proto(proto)),
            0x03 => Self::DapDisconnect(request::DapDisconnect::from_pdml_proto(proto)),
            0x04 => {
                Self::DapTransferConfigure(request::DapTransferConfigure::from_pdml_proto(proto))
            }
            0x05 => Self::DapTransfer(request::DapTransfer::from_pdml_proto(proto)),
            0x06 => Self::DapTransferBlock(request::DapTransferBlock::from_pdml_proto(proto)),
            0x08 => Self::DapWriteAbort(request::DapWriteAbort::from_pdml_proto(proto)),
            0x11 => Self::DapSwjClock(request::DapSwjClock::from_pdml_proto(proto)),
            0x12 => Self::DapSwjSequence(request::DapSwjSequence::from_pdml_proto(proto)),
            0x13 => Self::DapSwdConfigure(request::DapSwdConfigure::from_pdml_proto(proto)),
            header_byte => {
                log::warn!("Unknown CMSIS-DAP request? byte: {:#0X}", header_byte);
                Self::Unknown {
                    header_byte,
                    raw_data: raw_data_from_pdml_proto(proto),
                }
            }
        }
    }
}

impl request::DapConnect {
    pub fn from_pdml_proto(proto: pdml::Proto) -> Self {
        log::trace!("cmsis_dap.connect");
        let mut swd_port: Option<u8> = None;
        for field in proto.field.iter() {
            if field.name == "cmsis_dap.connect.port" {
                log::trace!("cmsis_dap.connect.port: {}", field.show);
                let value = u8::from_str_radix(field.show.trim_start_matches("0x"), 16).unwrap();
                assert!(
                    swd_port.replace(value).is_none(),
                    "Field shows up more than once"
                );
                break;
            }
        }

        let swd_port = swd_port.unwrap();
        Self { swd_port }
    }
}

impl request::DapDisconnect {
    pub fn from_pdml_proto(_proto: pdml::Proto) -> Self {
        log::trace!("cmsis_dap.disconnect");
        Self
    }
}

impl request::DapTransferConfigure {
    pub fn from_pdml_proto(proto: pdml::Proto) -> Self {
        log::trace!("cmsis_dap.transfer_config");
        let mut idle_cycles: Option<u8> = None;
        let mut wait_retry: Option<u16> = None;
        let mut match_retry: Option<u16> = None;
        for field in proto.field.iter() {
            if field.name == "cmsis_dap.transfer_config.idle_cycles" {
                log::trace!("cmsis_dap.transfer_config.idle_cycles: {}", field.show);
                let value = field.show.parse().unwrap();
                assert!(
                    idle_cycles.replace(value).is_none(),
                    "Field shows up more than once"
                );
            }
            if field.name == "cmsis_dap.transfer_config.wait_retry" {
                log::trace!("cmsis_dap.transfer_config.wait_retry: {}", field.show);
                let value = field.show.parse().unwrap();
                assert!(
                    wait_retry.replace(value).is_none(),
                    "Field shows up more than once"
                );
            }
            if field.name == "cmsis_dap.transfer_config.match_retry" {
                log::trace!("cmsis_dap.transfer_config.match_retry: {}", field.show);
                let value = field.show.parse().unwrap();
                assert!(
                    match_retry.replace(value).is_none(),
                    "Field shows up more than once"
                );
            }
        }

        // TODO: Maybe just unwrap °—°
        let idle_cycles = idle_cycles.unwrap();
        let wait_retry = wait_retry.unwrap();
        let match_retry = match_retry.unwrap();
        Self {
            idle_cycles,
            wait_retry,
            match_retry,
        }
    }
}

impl request::DapTransfer {
    pub fn from_pdml_proto(proto: pdml::Proto) -> Self {
        log::trace!("cmsis_dap.transfer");
        let mut dap_index: Option<u8> = None;
        let mut transfer_count: Option<NonZeroU8> = None;
        let mut transfers: Option<Vec<request::DapSingleTransfer>> = None;
        for field in proto.field.iter() {
            if field.name == "cmsis_dap.dap_index" {
                log::trace!("cmsis_dap.dap_index: {}", field.show);
                let value = field.show.parse().unwrap();
                assert!(
                    dap_index.replace(value).is_none(),
                    "Field shows up more than once"
                );
            }
            if field.name == "cmsis_dap.transfer.count" {
                log::trace!("cmsis_dap.transfer.count: {}", field.show);
                let value = field.show.parse().unwrap();
                assert!(
                    transfer_count.replace(value).is_none(),
                    "Field shows up more than once"
                );
            }
            if field.name == "cmsis_dap.transfer" {
                log::trace!("cmsis_dap.transfer: {}", field.show);
                assert!(
                    transfers.replace(Vec::new()).is_none(),
                    "Field shows up more than once"
                );
                let transfers = transfers.as_mut().unwrap();
                for field in field.field.iter() {
                    if field.name == "cmsis_dap.transfer.request" {
                        log::trace!("cmsis_dap.transfer.request: {}", field.show);
                        let value =
                            u8::from_str_radix(&field.show.trim_start_matches("0x"), 16).unwrap();
                        let request = request::DapTransferRequest::try_from(value).unwrap();
                        transfers.push(request::DapSingleTransfer {
                            request,
                            data: None,
                        })
                    }
                    if field.name == "cmsis_dap.transfer.write.data" {
                        log::trace!("cmsis_dap.transfer.write.data: {}", field.show);
                        let data: u32 = field.show.parse().unwrap();
                        // `write.data` always follows `request` it is referring to
                        // Thus, there must be something "last".
                        let last_transfer = transfers.last_mut().unwrap();
                        // Sanity check
                        let data_field_allowed = !last_transfer.request.rnw()
                            || last_transfer.request.match_mask()
                            || last_transfer.request.value_match();
                        assert!(
                            data_field_allowed,
                            "Data field is only allowed under certain conditions"
                        );
                        assert!(
                            last_transfer.data.replace(data).is_none(),
                            "Data field must not be occupied"
                        );
                    }
                }
            }
        }

        let dap_index = dap_index.unwrap();
        let transfer_count = transfer_count.unwrap();
        let transfers = transfers.unwrap();

        Self {
            dap_index,
            transfer_count,
            transfers,
        }
    }
}

impl request::DapTransferBlock {
    pub fn from_pdml_proto(proto: pdml::Proto) -> Self {
        log::trace!("cmsis_dap.transfer_block");
        let mut dap_index: Option<u8> = None;
        let mut transfer_count: Option<NonZeroU16> = None;
        let mut request: Option<request::DapTransferBlockRequest> = None;
        let mut data = Vec::new();
        for field in proto.field.iter() {
            if field.name == "cmsis_dap.dap_index" {
                log::trace!("cmsis_dap.dap_index: {}", field.show);
                let value = field.show.parse().unwrap();
                assert!(
                    dap_index.replace(value).is_none(),
                    "Field shows up more than once"
                );
            }
            if field.name == "cmsis_dap.transfer_block.count" {
                log::trace!("cmsis_dap.transfer_block.count: {}", field.show);
                let value = field.show.parse().unwrap();
                assert!(
                    transfer_count.replace(value).is_none(),
                    "Field shows up more than once"
                );
            }
            if field.name == "cmsis_dap.transfer.request" {
                log::trace!("cmsis_dap.transfer.request: {}", field.show);
                let value = u8::from_str_radix(&field.show.trim_start_matches("0x"), 16).unwrap();
                let value = request::DapTransferBlockRequest::try_from(value).unwrap();
                assert!(
                    request.replace(value).is_none(),
                    "Field shows up more than once"
                );
            }
            if field.name == "cmsis_dap.transfer.write.data" {
                log::trace!("cmsis_dap.transfer.write.data: {}", field.show);
                // `write.data` always follows `request` it is referring to
                // Thus, there must be something "last".
                let request = request.as_ref().unwrap();
                // Sanity check
                let data_field_allowed = !request.rnw();
                assert!(
                    data_field_allowed,
                    "Data field is only allowed under certain conditions"
                );
                let value: u32 = field.show.parse().unwrap();
                data.push(value);
            }
        }
        let dap_index = dap_index.unwrap();
        let transfer_count = transfer_count.unwrap();
        let request = request.unwrap();

        Self {
            dap_index,
            transfer_count,
            request,
            data,
        }
    }
}

impl request::DapWriteAbort {
    pub fn from_pdml_proto(proto: pdml::Proto) -> Self {
        log::trace!("cmsis_dap.write_abort");
        let mut dap_index: Option<u8> = None;
        let mut abort: Option<u32> = None;
        for field in proto.field.iter() {
            if field.name == "cmsis_dap.dap_index" {
                log::trace!("cmsis_dap.dap_index: {}", field.show);
                let value = field.show.parse().unwrap();
                assert!(
                    dap_index.replace(value).is_none(),
                    "Field shows up more than once"
                );
            }
            if field.name == "cmsis_dap.write_abort" {
                log::trace!("cmsis_dap.write_abort: {}", field.show);
                let value = field.show.parse().unwrap();
                assert!(
                    abort.replace(value).is_none(),
                    "Field shows up more than once"
                );
            }
        }

        let dap_index = dap_index.unwrap();
        let abort = abort.unwrap();

        Self { dap_index, abort }
    }
}

impl request::DapSwjClock {
    pub fn from_pdml_proto(proto: pdml::Proto) -> Self {
        log::trace!("cmsis_dap.swj_clock");
        let mut clock: Option<u32> = None;
        for field in proto.field.iter() {
            if field.name == "cmsis_dap.swj_clock" {
                log::trace!("cmsis_dap.swj_clock: {}", field.show);
                let value = field.show.parse().unwrap();
                assert!(
                    clock.replace(value).is_none(),
                    "Field shows up more than once"
                );
                break;
            }
        }

        let clock = clock.unwrap();
        Self { clock }
    }
}

impl request::DapSwjSequence {
    pub fn from_pdml_proto(proto: pdml::Proto) -> Self {
        log::trace!("cmsis_dap.swj_sequence");
        // 0 means 256
        let mut bit_count: Option<u8> = None;
        let mut bit_data: Option<String> = None;
        for field in proto.field.iter() {
            if field.name == "cmsis_dap.swj_sequence.count" {
                log::trace!("cmsis_dap.swj_sequence.count: {}", field.show);
                let value = field.show.parse().unwrap();
                assert!(
                    bit_count.replace(value).is_none(),
                    "Field shows up more than once"
                );
            }
            if field.name == "cmsis_dap.swj_sequence.data" {
                log::trace!("cmsis_dap.swj_sequence.data: {}", field.show);
                let value = field.show.clone();
                assert!(
                    bit_data.replace(value).is_none(),
                    "Field shows up more than once"
                );
            }
        }

        let bit_count = bit_count.unwrap();
        let bit_count: usize = if bit_count == 0 { 256 } else { bit_count as _ };
        let bit_data = bit_data.unwrap();
        let bit_data = bit_data
            .split(":")
            .map(|v| u8::from_str_radix(v, 16).unwrap())
            .take((bit_count + 7) / 8)
            .collect();
        Self {
            bit_count,
            bit_data,
        }
    }
}

impl request::DapSwdConfigure {
    pub fn from_pdml_proto(proto: pdml::Proto) -> Self {
        log::trace!("cmsis_dap.swd_config");
        let mut config: Option<u8> = None;
        for field in proto.field.iter() {
            if field.name == "cmsis_dap.swd_config" {
                log::trace!("cmsis_dap.swd_config: {}", field.show);
                let value = u8::from_str_radix(field.show.trim_start_matches("0x"), 16).unwrap();
                assert!(
                    config.replace(value).is_none(),
                    "Field shows up more than once"
                );
                break;
            }
        }

        let config = config.unwrap();
        let config = u3::try_new(config).unwrap();
        Self::from(config)
    }
}

impl TryFrom<u8> for response::DapResponseStatus {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::Ok),
            0xFF => Ok(Self::Err),
            value => Err(value),
        }
    }
}

impl response::DapResponseStatus {
    pub fn from_pdml_proto(proto: pdml::Proto) -> Self {
        {
            let mut status: Option<u8> = None;
            for field in proto.field.iter() {
                if field.name == "cmsis_dap.status" {
                    log::trace!("cmsis_dap.status: {}", field.show);
                    let value =
                        u8::from_str_radix(field.show.trim_start_matches("0x"), 16).unwrap();
                    assert!(
                        status.replace(value).is_none(),
                        "Field shows up more than once"
                    );
                    break;
                }
            }
            Self::try_from(status.unwrap()).unwrap()
        }
    }
}

impl response::DapConnect {
    pub fn from_pdml_proto(proto: pdml::Proto) -> Self {
        log::trace!("cmsis_dap.connect");
        let mut swd_port: Option<u8> = None;
        for field in proto.field.iter() {
            if field.name == "cmsis_dap.connect.port" {
                log::trace!("cmsis_dap.connect.port: {}", field.show);
                let value = u8::from_str_radix(field.show.trim_start_matches("0x"), 16).unwrap();
                assert!(
                    swd_port.replace(value).is_none(),
                    "Field shows up more than once"
                );
                break;
            }
        }

        let swd_port = swd_port.unwrap();
        Self { swd_port }
    }
}

impl response::DapDisconnect {
    pub fn from_pdml_proto(proto: pdml::Proto) -> Self {
        log::trace!("cmsis_dap.disconnect");
        let status = response::DapResponseStatus::from_pdml_proto(proto);
        Self { status }
    }
}

impl response::DapTransferConfigure {
    pub fn from_pdml_proto(proto: pdml::Proto) -> Self {
        log::trace!("cmsis_dap.transfer_config");
        let status = response::DapResponseStatus::from_pdml_proto(proto);
        Self { status }
    }
}

impl response::DapTransfer {
    pub fn from_pdml_proto(proto: pdml::Proto) -> Self {
        log::trace!("cmsis_dap.transfer");
        let mut transfer_count: Option<u8> = None;
        let mut response: Option<response::DapTransferResponse> = None;
        let mut data = Vec::new();
        for field in proto.field.iter() {
            if field.name == "cmsis_dap.transfer.count" {
                log::trace!("cmsis_dap.transfer.count: {}", field.show);
                let value = field.show.parse().unwrap();
                assert!(
                    transfer_count.replace(value).is_none(),
                    "Field shows up more than once"
                );
            }
            if field.name == "cmsis_dap.transfer.response" {
                log::trace!("cmsis_dap.transfer.response: {}", field.show);
                let value = u8::from_str_radix(field.show.trim_start_matches("0x"), 16).unwrap();
                let value = response::DapTransferResponse::try_from(value).unwrap();
                assert!(
                    response.replace(value).is_none(),
                    "Field shows up more than once"
                );
            }
            if field.name == "cmsis_dap.transfer.read.data" {
                log::trace!("cmsis_dap.transfer.read.data: {}", field.show);
                let value = field.show.parse().unwrap();
                data.push(value);
            }
        }

        let transfer_count = transfer_count.unwrap();
        let response = response.unwrap();

        Self {
            transfer_count,
            response,
            data,
        }
    }
}

impl response::DapTransferBlock {
    pub fn from_pdml_proto(proto: pdml::Proto) -> Self {
        {
            log::trace!("cmsis_dap.transfer_block");
            let mut transfer_count: Option<u8> = None;
            let mut response: Option<response::DapTransferBlockResponse> = None;
            let mut data = Vec::new();
            for field in proto.field.iter() {
                if field.name == "cmsis_dap.transfer_block.count" {
                    log::trace!("cmsis_dap.transfer_block.count: {}", field.show);
                    let value = field.show.parse().unwrap();
                    assert!(
                        transfer_count.replace(value).is_none(),
                        "Field shows up more than once"
                    );
                }
                if field.name == "cmsis_dap.transfer.response" {
                    log::trace!("cmsis_dap.transfer.response: {}", field.show);
                    let value =
                        u8::from_str_radix(field.show.trim_start_matches("0x"), 16).unwrap();
                    let value = response::DapTransferBlockResponse::try_from(value).unwrap();
                    assert!(
                        response.replace(value).is_none(),
                        "Field shows up more than once"
                    );
                }
                if field.name == "cmsis_dap.transfer.read.data" {
                    log::trace!("cmsis_dap.transfer.read.data: {}", field.show);
                    let value = field.show.parse().unwrap();
                    data.push(value);
                }
            }

            let transfer_count = transfer_count.unwrap();
            let response = response.unwrap();

            Self {
                transfer_count,
                response,
                data,
            }
        }
    }
}

impl response::DapWriteAbort {
    pub fn from_pdml_proto(proto: pdml::Proto) -> Self {
        log::trace!("cmsis_dap.write_abort");
        let status = response::DapResponseStatus::from_pdml_proto(proto);
        Self { status }
    }
}

impl response::DapSwjClock {
    pub fn from_pdml_proto(proto: pdml::Proto) -> Self {
        log::trace!("cmsis_dap.swj_clock");
        let status = response::DapResponseStatus::from_pdml_proto(proto);
        Self { status }
    }
}

impl response::DapSwjSequence {
    pub fn from_pdml_proto(proto: pdml::Proto) -> Self {
        log::trace!("cmsis_dap.swj_sequence");
        let status = response::DapResponseStatus::from_pdml_proto(proto);
        Self { status }
    }
}

impl response::DapSwdConfigure {
    pub fn from_pdml_proto(proto: pdml::Proto) -> Self {
        log::trace!("cmsis_dap.swd_config");
        let status = response::DapResponseStatus::from_pdml_proto(proto);
        Self { status }
    }
}
