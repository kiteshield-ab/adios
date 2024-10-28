pub(crate) mod cmsis_dap;
pub(crate) mod pdml;

use std::{fmt::Display, io::BufRead, rc::Rc};

use adios_common::{Command, Input};
use bilge::prelude::*;
use cmsis_dap::{
    response::{DapResponseStatus, DapTransferResponseAck},
    Frame, Request, Response,
};

pub fn generate_vm_input(r: impl BufRead) -> Vec<Input> {
    let pdml: pdml::Pdml = quick_xml::de::from_reader(r).unwrap();

    #[derive(Clone, Debug)]
    struct AwaitingRequest {
        number: usize,
        content: Request,
        corresponding_response: usize,
    }

    let mut adi_commands = Vec::new();
    let mut request_waiting: Option<AwaitingRequest> = None;
    for packet in pdml.packet.into_iter() {
        let Some(frame) = Frame::from_pdml_packet(&packet) else {
            continue;
        };
        log::debug!("{:#?}", frame);
        match (&request_waiting, &frame.content) {
            // Request is pending. Frame is expected to be a response.
            (
                Some(request),
                cmsis_dap::Content::CmsisDapResponse {
                    content: response_content,
                    corresponding_request,
                },
            ) => {
                if request.number != *corresponding_request {
                    log::warn!(
                        "Frame numbers are not matching (res->req: {}, req: {})",
                        corresponding_request,
                        request.number
                    );
                    // Proceed?
                    request_waiting = None;
                    continue;
                }
                if frame.number != request.corresponding_response {
                    log::warn!(
                        "Frame numbers are not matching (res: {}, req->res: {})",
                        frame.number,
                        request.corresponding_response
                    );
                    // Proceed?
                    request_waiting = None;
                    continue;
                }
                match (&request.content, response_content) {
                    (Request::DapTransfer(req), Response::DapTransfer(res)) => {
                        log::info!("Request ({}): {:#0X?}", request.number, req);
                        log::info!("Response ({}): {:#0X?}", frame.number, res);
                        if matches!(
                            res.response.ack(),
                            DapTransferResponseAck::Fault | DapTransferResponseAck::NoAck
                        ) || res.response.protocol_error()
                            || res.response.value_mismatch()
                        {
                            log::warn!("Response ({}) is faulty, skipping", frame.number);
                            request_waiting = None;
                            continue;
                        }
                        // If Ack.Ok then `req.transfer_count == res.transfer_count`
                        // If Ack.Wait, then last valid transfer is `res.transfer_count - 1`
                        let valid_transfers = match res.response.ack() {
                            DapTransferResponseAck::Ok => res.transfer_count,
                            DapTransferResponseAck::Wait => {
                                res.transfer_count.saturating_sub(1)
                            }
                            _ => unreachable!(),
                        };
                        let mut read_data_iter = res.data.iter();
                        for index in 0..valid_transfers as usize {
                            let transfer = &req.transfers[index];
                            let data = if transfer.request.rnw() {
                                *read_data_iter.next().unwrap()
                            } else {
                                transfer.data.unwrap()
                            };
                            let a = u2::new(
                                ((transfer.request.a3() as u8) << 1)
                                    | (transfer.request.a2() as u8),
                            );
                            adi_commands.push(
                                Command {
                                    ts: None,
                                    apndp: transfer.request.apndp(),
                                    rnw: transfer.request.rnw(),
                                    a,
                                    data,
                                }
                                .into(),
                            );
                        }
                    }
                    (Request::DapTransferBlock(req), Response::DapTransferBlock(res)) => {
                        log::info!("Request ({}): {:#0X?}", request.number, req);
                        log::info!("Response ({}): {:#0X?}", frame.number, res);
                        if matches!(
                            res.response.ack(),
                            DapTransferResponseAck::Fault | DapTransferResponseAck::NoAck
                        ) || res.response.protocol_error()
                        {
                            log::warn!("Response ({}) is faulty, skipping", frame.number);
                            request_waiting = None;
                            continue;
                        }
                        // If Ack.Ok then `req.transfer_count == res.transfer_count`
                        // If Ack.Wait, then last valid transfer is `res.transfer_count - 1`
                        let valid_transfers = match res.response.ack() {
                            DapTransferResponseAck::Ok => res.transfer_count,
                            DapTransferResponseAck::Wait => {
                                res.transfer_count.saturating_sub(1)
                            }
                            _ => unreachable!(),
                        } as usize;
                        let data_source = if req.request.rnw() {
                            res.data.iter().copied()
                        } else {
                            req.data.iter().copied()
                        };

                        let a =
                            u2::new(((req.request.a3() as u8) << 1) | (req.request.a2() as u8));
                        for data in data_source.take(valid_transfers) {
                            adi_commands.push(
                                Command {
                                    ts: None,
                                    apndp: req.request.apndp(),
                                    rnw: req.request.rnw(),
                                    a,
                                    data,
                                }
                                .into(),
                            );
                        }
                    }
                    (Request::DapWriteAbort(req), Response::DapWriteAbort(res)) => {
                        log::info!("Request ({}): {:#0X?}", request.number, req);
                        log::info!("Response ({}): {:#0X?}", frame.number, res);
                        if let DapResponseStatus::Ok = res.status {
                            adi_commands.push(
                                Command {
                                    ts: None,
                                    apndp: false,
                                    rnw: false,
                                    a: u2::new(0b00),
                                    data: req.abort,
                                }
                                .into(),
                            );
                        } else {
                            log::warn!("Response ({}) is Err, skipping", frame.number);
                        }
                    }
                    (
                        Request::Unknown {
                            header_byte,
                            raw_data: request_data,
                        },
                        Response::Unknown {
                            raw_data: response_data,
                            ..
                        },
                    ) => {
                        let metadata = UnknownCommandLandmark {
                            header_byte: *header_byte,
                            request_data: request_data.clone(),
                            response_data: response_data.clone(),
                        };
                        let metadata = Rc::new(metadata);
                        adi_commands.push(Input::Landmark { metadata });
                    }
                    (req, res) => {
                        log::info!("Request ({}): {:#0X?}", request.number, req);
                        log::info!("Response ({}): {:#0X?}", frame.number, res);
                    }
                }
                request_waiting = None;
            }
            (
                None,
                cmsis_dap::Content::CmsisDapRequest {
                    content,
                    corresponding_response,
                },
            ) => {
                // No request is pending. Frame is expected to be a request.
                request_waiting = Some(AwaitingRequest {
                    number: frame.number,
                    content: content.clone(),
                    corresponding_response: *corresponding_response,
                });
            }
            (_cr, _fc) => {
                log::error!("Invalid state");
                unimplemented!();
            }
        }
    }
    adi_commands
}

pub struct UnknownCommandLandmark {
    pub header_byte: u8,
    pub request_data: Vec<u8>,
    pub response_data: Vec<u8>,
}

impl Display for UnknownCommandLandmark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CMSIS-DAP Unknown <{:#0X}> / req: {:0X?} / res: {:0X?}",
            &self.header_byte,
            &&self.request_data[0..self.request_data.len().min(5)],
            &&self.response_data[0..self.response_data.len().min(5)],
        )
    }
}
