use adios_common::{Command, Timestamp};
use bilge::prelude::*;
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{digit1, hex_digit1, line_ending},
    combinator::{all_consuming, eof, fail, map_res},
    multi::{many0, many0_count, many1},
    sequence::pair,
    IResult, Parser,
};

#[derive(Copy, Clone)]
enum AccessId {
    Dp(Dp),
    Ap(Ap),
}

impl AccessId {
    fn tag(&self) -> impl FnMut(&str) -> IResult<&str, &str> + '_ {
        move |input| match self {
            AccessId::Dp(v) => v.tag()(input),
            AccessId::Ap(v) => v.tag()(input),
        }
    }

    fn simple(input: &str) -> IResult<&str, Self> {
        if Self::complex(input).is_ok() {
            fail(input)
        } else {
            alt((Dp::parse, Ap::w))(input)
        }
    }

    fn complex(input: &str) -> IResult<&str, Self> {
        Ap::r(input)
    }

    fn command(&self, data: u32, ts: Timestamp) -> Command {
        let apndp;
        let rnw;
        let a = match self {
            AccessId::Dp(dp) => {
                apndp = false;
                match dp {
                    Dp::R(r) => {
                        rnw = true;
                        (*r as u8) >> 2
                    }
                    Dp::W(w) => {
                        rnw = false;
                        (*w as u8) >> 2
                    }
                }
            }
            AccessId::Ap(ap) => {
                apndp = true;
                rnw = matches!(ap, Ap::R(_));
                let (Ap::R(v) | Ap::W(v)) = ap;
                (*v as u8) >> 2
            }
        };
        let a = u2::new(a);
        Command {
            apndp,
            rnw,
            a,
            data,
            ts: Some(ts),
        }
    }
}

#[derive(Copy, Clone)]
enum Dp {
    R(ReadDp),
    W(WriteDp),
}

impl Dp {
    fn tag(&self) -> impl FnMut(&str) -> IResult<&str, &str> + '_ {
        move |input| match self {
            Dp::R(ReadDp::IdCode) => tag("IDCODE")(input),
            Dp::R(ReadDp::CtrlStat) => tag("R CTRL/STAT")(input),
            Dp::R(ReadDp::Resend) => tag("RESEND")(input),
            Dp::R(ReadDp::Rdbuff) => tag("RDBUFF")(input),
            Dp::W(WriteDp::Abort) => tag("W ABORT")(input),
            Dp::W(WriteDp::CtrlStat) => tag("W CTRL/STAT")(input),
            Dp::W(WriteDp::Select) => tag("W SELECT")(input),
        }
    }
    fn rdbuff(input: &str) -> IResult<&str, AccessId> {
        tag("RDBUFF")
            .map(|_| AccessId::Dp(Self::R(ReadDp::Rdbuff)))
            .parse(input)
    }
    fn parse(input: &str) -> IResult<&str, AccessId> {
        alt((
            tag("IDCODE").map(|_| AccessId::Dp(Self::R(ReadDp::IdCode))),
            tag("R CTRL/STAT").map(|_| AccessId::Dp(Self::R(ReadDp::CtrlStat))),
            tag("RESEND").map(|_| AccessId::Dp(Self::R(ReadDp::Resend))),
            Self::rdbuff,
            tag("W ABORT").map(|_| AccessId::Dp(Self::W(WriteDp::Abort))),
            tag("W CTRL/STAT").map(|_| AccessId::Dp(Self::W(WriteDp::CtrlStat))),
            tag("W SELECT").map(|_| AccessId::Dp(Self::W(WriteDp::Select))),
        ))(input)
    }
}

#[derive(Copy, Clone)]
enum ReadDp {
    IdCode = 0x0,
    CtrlStat = 0x4,
    Resend = 0x8,
    Rdbuff = 0xC,
}

#[derive(Copy, Clone)]
enum WriteDp {
    Abort = 0x0,
    CtrlStat = 0x4,
    Select = 0x8,
}

#[derive(Copy, Clone)]
enum Ap {
    R(InnerAp),
    W(InnerAp),
}

impl Ap {
    fn tag(&self) -> impl FnMut(&str) -> IResult<&str, &str> + '_ {
        move |input| match self {
            Ap::R(InnerAp::One) => tag("R AP0")(input),
            Ap::R(InnerAp::Two) => tag("R AP4")(input),
            Ap::R(InnerAp::Three) => tag("R AP8")(input),
            Ap::R(InnerAp::Four) => tag("R APc")(input),
            Ap::W(InnerAp::One) => tag("W AP0")(input),
            Ap::W(InnerAp::Two) => tag("W AP4")(input),
            Ap::W(InnerAp::Three) => tag("W AP8")(input),
            Ap::W(InnerAp::Four) => tag("W APc")(input),
        }
    }
    fn r(input: &str) -> IResult<&str, AccessId> {
        let (input, _) = tag("R AP")(input)?;
        let (input, hex) = InnerAp::parse(input)?;
        Ok((input, AccessId::Ap(Self::R(hex))))
    }

    fn w(input: &str) -> IResult<&str, AccessId> {
        let (input, _) = tag("W AP")(input)?;
        let (input, hex) = InnerAp::parse(input)?;
        Ok((input, AccessId::Ap(Self::W(hex))))
    }
}

#[derive(Copy, Clone)]
enum InnerAp {
    One = 0x0,
    Two = 0x4,
    Three = 0x8,
    Four = 0xc,
}

impl InnerAp {
    fn parse(input: &str) -> IResult<&str, Self> {
        alt((
            tag("0").map(|_| Self::One),
            tag("4").map(|_| Self::Two),
            tag("8").map(|_| Self::Three),
            tag("c").map(|_| Self::Four),
        ))(input)
    }
}

pub fn generate_vm_commands(input: &str) -> Result<Vec<Command>, nom::Err<nom::error::Error<&str>>> {
    let (_, commands) = all_consuming(many1(command))(input)?;
    Ok(commands.into_iter().flat_map(|v| v).collect())
}

fn command(input: &str) -> IResult<&str, Vec<Command>> {
    alt((simple_command, complex_command, ll::ignored_commands))(input)
}

fn simple_command(input: &str) -> IResult<&str, Vec<Command>> {
    let (input, ll_command) = ll::command(AccessId::simple, true)(input)?;
    // Do not propagate certain accesses.
    // - RDBUFF is relevant only if following AP read operation (complex command) and then it should be merged with it
    let command = match ll_command {
        Some(command) => match &command.access_id {
            AccessId::Dp(Dp::R(ReadDp::Rdbuff)) => None,
            _ => Some(command.into()),
        },
        None => None,
    };
    Ok((input, command.into_iter().collect()))
}

fn complex_command(input: &str) -> IResult<&str, Vec<Command>> {
    let (input, ll_commands) = many1(ll::command(AccessId::complex, false))(input)?;
    if ll_commands.iter().any(|v| v.is_none()) {
        // TODO: No real-life example of this, hard to determine how to handle it
        log::error!("R APx FAULTs? Parsing might be incomplete");
        // Cleanup? All of this is theoretical
        let (input, _) = many0(ll::command(Dp::rdbuff, true))(input)?;
        return Ok((input, Vec::new()));
    }

    let ll_commands: Vec<_> = ll_commands.into_iter().filter_map(|v| v).collect();

    let (input, rdbuff_ll_command) = ll::command(Dp::rdbuff, true)(input)?;
    let Some(rdbuff_ll_command) = rdbuff_ll_command else {
        return Ok((input, Vec::new()));
    };

    let commands = ll_commands
        .iter()
        .zip(
            ll_commands
                .iter()
                .skip(1) // Offset by one
                .chain(core::iter::once(&rdbuff_ll_command)),
        )
        .map(|(l, r)| {
            // Offset by one
            ll::Command {
                ts: Timestamp {
                    start: l.ts.start,
                    end: r.ts.end,
                },
                access_id: l.access_id,
                value: r.value,
            }
            .into()
        })
        .collect();

    Ok((input, commands))
}

mod ll {
    use super::*;
    pub(super) struct Command {
        pub(super) ts: Timestamp,
        pub(super) access_id: AccessId,
        pub(super) value: u32,
    }

    impl From<Command> for super::Command {
        fn from(cmd: Command) -> Self {
            let Command {
                ts,
                access_id,
                value,
            } = cmd;
            access_id.command(value, ts)
        }
    }

    pub fn command(
        command: impl FnMut(&str) -> IResult<&str, AccessId> + Copy,
        eof_ok: bool,
    ) -> impl FnMut(&str) -> IResult<&str, Option<Command>> {
        move |input| {
            enum Ack {
                Ok,
                Wait,
                Fault,
            }
            let (input, (start_ts, access_id)) = line(command, false)(input)?;
            let (input, (.., ack)) = alt((
                // TODO: Make `ll::line` work with all Parser::map
                // For now, this is a workaround
                line(alt((tag("OK").map(|_| Ack::Ok), fail)), false),
                line(alt((tag("WAIT").map(|_| Ack::Wait), fail)), false),
                line(alt((tag("FAULT").map(|_| Ack::Fault), fail)), true),
            ))(input)?;
            let (input, ack) = if let Ack::Wait = ack {
                let (input, _) =
                    many0(pair(line(access_id.tag(), false), line(tag("WAIT"), true)))(input)?;
                let (input, count) = many0_count(line(access_id.tag(), false))(input)?;
                if count == 0 {
                    return Ok((input, None));
                }
                // Very improbable case in real life. Usually when WAIT occurs, SWD is screwed up.
                let (input, (.., ack)) = line(
                    alt((tag("OK").map(|_| Ack::Ok), tag("FAULT").map(|_| Ack::Fault))),
                    false,
                )(input)?;
                (input, ack)
            } else {
                (input, ack)
            };
            match ack {
                Ack::Ok => {
                    let (input, (end_ts, value)) = line(value, eof_ok)(input)?;
                    Ok((
                        input,
                        Some(Command {
                            ts: Timestamp {
                                start: start_ts.start,
                                end: end_ts.end,
                            },
                            value,
                            access_id,
                        }),
                    ))
                }
                Ack::Wait => {
                    unreachable!("Another WAIT? Come on.");
                }
                Ack::Fault => Ok((input, None)),
            }
        }
    }

    fn line<'a: 'b, 'b, T>(
        mut command: impl FnMut(&'a str) -> IResult<&'b str, T>,
        eof_ok: bool,
    ) -> impl FnMut(&'a str) -> IResult<&'b str, (Timestamp, T)> {
        move |input| {
            let (input, ts) = timestamps(input)?;
            let (input, access_id) = command(input)?;
            let input = if eof_ok {
                let (input, _) = alt((line_ending, eof))(input)?;
                input
            } else {
                let (input, _) = line_ending(input)?;
                input
            };
            Ok((input, (ts, access_id)))
        }
    }

    // TODO: Maybe VM should be reset when LINERESET encountered.
    // OTOH, CMSIS_DAP based VM does nothing on DAP_Connect
    pub fn ignored_commands(input: &str) -> IResult<&str, Vec<super::Command>> {
        let (input, _) =
            alt((line(tag("LINERESET"), false), line(tag("JTAG->SWD"), false)))(input)?;
        Ok((input, Vec::new()))
    }

    fn value(input: &str) -> IResult<&str, u32> {
        let (input, _) = tag("0x")(input)?;
        let (input, hex) = map_res(hex_digit1, hex_u32)(input)?;
        Ok((input, hex))
    }

    fn timestamps(input: &str) -> IResult<&str, Timestamp> {
        let (input, start) = dec_u64(input)?;
        let (input, _) = tag("-")(input)?;
        let (input, end) = dec_u64(input)?;
        let (input, _) = tag(" swd-1: ")(input)?;
        Ok((input, Timestamp { start, end }))
    }

    fn hex_u32(input: &str) -> Result<u32, std::num::ParseIntError> {
        u32::from_str_radix(input, 16)
    }

    fn dec_u64(input: &str) -> IResult<&str, u64> {
        map_res(digit1, str::parse)(input)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn simple_command_with_ok() {
        let text_sample = "17-1337 swd-1: IDCODE
1337-1337 swd-1: OK
1337-71 swd-1: 0x5ba02477
";
        let commands = generate_vm_commands(text_sample).unwrap();
        let expected_commands = [Command {
            ts: Some(Timestamp { start: 17, end: 71 }),
            apndp: false,
            rnw: true,
            a: u2::new(0),
            data: 0x5ba02477,
        }];
        assert_eq!(commands.len(), expected_commands.len());
        commands
            .into_iter()
            .zip(expected_commands.into_iter())
            .for_each(|(ac, ex)| assert_eq!(ac, ex));
    }

    #[test]
    fn simple_command_with_wait_with_ok() {
        let text_sample = "17-1337 swd-1: IDCODE
1337-1337 swd-1: WAIT
1337-1337 swd-1: IDCODE
1337-1337 swd-1: WAIT
1337-1337 swd-1: IDCODE
1337-1337 swd-1: WAIT
1337-1337 swd-1: IDCODE
1337-1337 swd-1: WAIT
1337-1337 swd-1: IDCODE
1337-1337 swd-1: WAIT
1337-1337 swd-1: IDCODE
1337-1337 swd-1: OK
1337-71 swd-1: 0x5ba02477
";
        let commands = generate_vm_commands(text_sample).unwrap();
        let expected_commands = [Command {
            ts: Some(Timestamp { start: 17, end: 71 }),
            apndp: false,
            rnw: true,
            a: u2::new(0),
            data: 0x5ba02477,
        }];
        assert_eq!(commands.len(), expected_commands.len());
        commands
            .into_iter()
            .zip(expected_commands.into_iter())
            .for_each(|(ac, ex)| assert_eq!(ac, ex));
    }

    #[test]
    fn simple_command_with_wait_with_interrupt() {
        let text_sample = "1337-1337 swd-1: IDCODE
1337-1337 swd-1: WAIT
1337-1337 swd-1: IDCODE
1337-1337 swd-1: WAIT
1337-1337 swd-1: IDCODE
1337-1337 swd-1: WAIT
1337-1337 swd-1: IDCODE
1337-1337 swd-1: WAIT
1337-1337 swd-1: LINERESET
1337-1337 swd-1: JTAG->SWD
1337-1337 swd-1: LINERESET
17-1337 swd-1: IDCODE
1337-1337 swd-1: WAIT
1337-1337 swd-1: IDCODE
1337-1337 swd-1: OK
1337-71 swd-1: 0x5ba02477
";
        let commands = generate_vm_commands(text_sample).unwrap();
        let expected_commands = [Command {
            ts: Some(Timestamp { start: 17, end: 71 }),
            apndp: false,
            rnw: true,
            a: u2::new(0),
            data: 0x5ba02477,
        }];
        assert_eq!(commands.len(), expected_commands.len());
        commands
            .into_iter()
            .zip(expected_commands.into_iter())
            .for_each(|(ac, ex)| assert_eq!(ac, ex));
    }

    #[test]
    fn simple_command_with_wait_and_switch() {
        let text_sample = "1337-1337 swd-1: IDCODE
1337-1337 swd-1: WAIT
1337-1337 swd-1: IDCODE
1337-1337 swd-1: WAIT
1337-1337 swd-1: IDCODE
1337-1337 swd-1: WAIT
1337-1337 swd-1: W SELECT
1337-1337 swd-1: WAIT
1337-1337 swd-1: W SELECT
1337-1337 swd-1: WAIT
1337-1337 swd-1: IDCODE
1337-1337 swd-1: WAIT
1337-1337 swd-1: IDCODE
1337-1337 swd-1: WAIT
17-1337 swd-1: W ABORT
1337-1337 swd-1: OK
1337-71 swd-1: 0xdeadbeef
";
        let commands = generate_vm_commands(text_sample).unwrap();
        let expected_commands = [Command {
            ts: Some(Timestamp { start: 17, end: 71 }),
            apndp: false,
            rnw: false,
            a: u2::new(0),
            data: 0xdeadbeef,
        }];
        assert_eq!(commands.len(), expected_commands.len());
        commands
            .into_iter()
            .zip(expected_commands.into_iter())
            .for_each(|(ac, ex)| assert_eq!(ac, ex));
    }

    #[test]
    fn ignore_unsolicited_rdbuffs() {
        let text_sample = "1337-1337 swd-1: LINERESET
1337-1337 swd-1: JTAG->SWD
1337-1337 swd-1: LINERESET
17-1337 swd-1: IDCODE
1337-1337 swd-1: OK
1337-71 swd-1: 0x1
1337-1337 swd-1: RDBUFF
1337-1337 swd-1: OK
1337-1337 swd-1: 0x01100001";
        let commands = generate_vm_commands(text_sample).unwrap();
        let expected_commands = [Command {
            ts: Some(Timestamp { start: 17, end: 71 }),
            apndp: false,
            rnw: true,
            a: u2::new(0),
            data: 0x1,
        }];
        assert_eq!(commands.len(), expected_commands.len());
        commands
            .into_iter()
            .zip(expected_commands.into_iter())
            .for_each(|(ac, ex)| assert_eq!(ac, ex));
    }
    #[test]
    fn ignore_faults() {
        let text_sample = "1337-1337 swd-1: LINERESET
1337-1337 swd-1: JTAG->SWD
1337-1337 swd-1: LINERESET
17-1337 swd-1: IDCODE
1337-1337 swd-1: OK
1337-71 swd-1: 0x1
1337-1337 swd-1: R APc
1337-1337 swd-1: FAULT";
        let commands = generate_vm_commands(text_sample).unwrap();
        let expected_commands = [Command {
            ts: Some(Timestamp { start: 17, end: 71 }),
            apndp: false,
            rnw: true,
            a: u2::new(0),
            data: 0x1,
        }];
        assert_eq!(commands.len(), expected_commands.len());
        commands
            .into_iter()
            .zip(expected_commands.into_iter())
            .for_each(|(ac, ex)| assert_eq!(ac, ex));
    }

    #[test]
    fn chained_ap_reads() {
        let text_sample = "12-1337 swd-1: R AP0
1337-1337 swd-1: OK
1337-1337 swd-1: 0x00000000
13-1337 swd-1: R AP4
1337-1337 swd-1: OK
1337-21 swd-1: 0x00000001
14-1337 swd-1: R AP8
1337-1337 swd-1: OK
1337-31 swd-1: 0x00000002
15-1337 swd-1: R APc
1337-1337 swd-1: OK
1337-41 swd-1: 0x00000003
1337-1337 swd-1: RDBUFF
1337-1337 swd-1: OK
1337-51 swd-1: 0x00000004";
        let commands = generate_vm_commands(text_sample).unwrap();
        let expected_commands = [
            Command {
                ts: Some(Timestamp { start: 12, end: 21 }),
                apndp: true,
                rnw: true,
                a: u2::new(0),
                data: 0x1,
            },
            Command {
                ts: Some(Timestamp { start: 13, end: 31 }),
                apndp: true,
                rnw: true,
                a: u2::new(1),
                data: 0x2,
            },
            Command {
                ts: Some(Timestamp { start: 14, end: 41 }),
                apndp: true,
                rnw: true,
                a: u2::new(2),
                data: 0x3,
            },
            Command {
                ts: Some(Timestamp { start: 15, end: 51 }),
                apndp: true,
                rnw: true,
                a: u2::new(3),
                data: 0x4,
            },
        ];
        assert_eq!(commands.len(), expected_commands.len());
        commands
            .into_iter()
            .zip(expected_commands.into_iter())
            .for_each(|(ac, ex)| assert_eq!(ac, ex));
    }

    #[test]
    fn single_ap_reads() {
        let text_sample = "12-1337 swd-1: R AP0
1337-1337 swd-1: OK
1337-1337 swd-1: 0xFFFFFFFF
1337-1337 swd-1: RDBUFF
1337-1337 swd-1: OK
1337-21 swd-1: 0x00000000
13-1337 swd-1: R AP4
1337-1337 swd-1: OK
1337-1337 swd-1: 0xFFFFFFFF
1337-1337 swd-1: RDBUFF
1337-1337 swd-1: OK
1337-31 swd-1: 0x00000001
14-1337 swd-1: R AP8
1337-1337 swd-1: OK
1337-1337 swd-1: 0xFFFFFFFF
1337-1337 swd-1: RDBUFF
1337-1337 swd-1: OK
1337-41 swd-1: 0x00000002
15-1337 swd-1: R APc
1337-1337 swd-1: OK
1337-1337 swd-1: 0xFFFFFFFF
1337-1337 swd-1: RDBUFF
1337-1337 swd-1: OK
1337-51 swd-1: 0x00000003
";
        let commands = generate_vm_commands(text_sample).unwrap();
        let expected_commands = [
            Command {
                ts: Some(Timestamp { start: 12, end: 21 }),
                apndp: true,
                rnw: true,
                a: u2::new(0),
                data: 0x0,
            },
            Command {
                ts: Some(Timestamp { start: 13, end: 31 }),
                apndp: true,
                rnw: true,
                a: u2::new(1),
                data: 0x1,
            },
            Command {
                ts: Some(Timestamp { start: 14, end: 41 }),
                apndp: true,
                rnw: true,
                a: u2::new(2),
                data: 0x2,
            },
            Command {
                ts: Some(Timestamp { start: 15, end: 51 }),
                apndp: true,
                rnw: true,
                a: u2::new(3),
                data: 0x3,
            },
        ];
        assert_eq!(commands.len(), expected_commands.len());
        commands
            .into_iter()
            .zip(expected_commands.into_iter())
            .for_each(|(ac, ex)| assert_eq!(ac, ex));
    }
}
