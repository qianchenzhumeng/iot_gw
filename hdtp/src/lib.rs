
use std::fmt;
use log::error;

const MAX_LENGTH: usize = 255;

enum MechainState {
    SearchingForFlag,
    ReceivingLength,
    ReceivingPayload,
    ReceivingFcs,
}

enum FcsState {
    Start,
    End,
}

pub struct Hdtp {
    rx_frame_state: MechainState,
    rx_frame_length: u8,
    rx_msg_is_ready: bool,
    rx_frame_buffer: [u8; MAX_LENGTH],
    rx_frame_payload_bytes: u8,
    rx_fcs_state: FcsState,
    rx_frame_fcs: u16,
}

impl fmt::Display for MechainState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MechainState::SearchingForFlag => write!(f, "SearchingForFlag"),
            MechainState::ReceivingLength => write!(f, "ReceivingLength"),
            MechainState::ReceivingPayload => write!(f, "ReceivingPayload"),
            MechainState::ReceivingFcs => write!(f, "ReceivingFcs"),
        }
    }
}

impl fmt::Display for FcsState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FcsState::Start => write!(f, "Start"),
            FcsState::End => write!(f, "End"),
        }
    }
}

impl fmt::Display for Hdtp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let slice: &[u8; MAX_LENGTH] = &self.rx_frame_buffer;
        write!(f, "{{")?;
        write!(f, "rx_frame_state: {}, rx_frame_length: {}, rx_msg_is_ready: {}, ", self.rx_frame_state, self.rx_frame_length, self.rx_msg_is_ready)?;
        write!(f, "rx_frame_buffer: [")?;
        for i in 0..(MAX_LENGTH - 1) {
            write!(f, "{}, ", slice[i as usize])?;
        }
        write!(f, "{}], ", slice[(MAX_LENGTH - 1)as usize])?;
        write!(f, "rx_frame_payload_bytes: {}, ", self.rx_frame_payload_bytes)?;
        write!(f, "rx_fcs_state: {}, ", self.rx_fcs_state)?;
        write!(f, "rx_frame_fcs: {}", self.rx_frame_fcs)?;
        write!(f, "}}")?;
        Ok(())
    }
}

impl Hdtp {
    pub fn new() -> Self {
        Hdtp {
            rx_frame_state: MechainState::SearchingForFlag,
            rx_frame_length: 0,
            rx_msg_is_ready: false,
            rx_frame_buffer: [0_u8; MAX_LENGTH],
            rx_frame_payload_bytes: 0,
            rx_fcs_state: FcsState::Start,
            rx_frame_fcs: 0,
        }
    }

    pub fn input(&mut self, c: u8) {
        match self.rx_frame_state {
            MechainState::SearchingForFlag => {
                self.rx_frame_payload_bytes = 0;
                self.rx_frame_length = 0;
                self.rx_msg_is_ready = false;
                self.rx_frame_fcs = 0;
                if c == 0x7E {
                    self.rx_frame_state = MechainState::ReceivingLength;
                }
            },
            MechainState::ReceivingLength => {
                self.rx_frame_length = c;
                self.rx_frame_state = MechainState::ReceivingPayload;
            },
            MechainState::ReceivingPayload => {
                let slice: &mut [u8; MAX_LENGTH] = &mut self.rx_frame_buffer;
                slice[self.rx_frame_payload_bytes as usize] = c;
                self.rx_frame_payload_bytes += 1;
                if self.rx_frame_length == self.rx_frame_payload_bytes {
                    self.rx_frame_state = MechainState::ReceivingFcs;
                }
            },
            MechainState::ReceivingFcs => {
                match self.rx_fcs_state {
                    FcsState::Start => {
                        self.rx_frame_fcs |= c as u16;
                        self.rx_fcs_state = FcsState::End;
                    },
                    FcsState::End => {
                        self.rx_frame_fcs = self.rx_frame_fcs << 8 | c as u16;
                        self.rx_fcs_state = FcsState::Start;
                        self.handle_msg();
                        self.rx_frame_state = MechainState::SearchingForFlag;
                    },
                }
            },
        }
    }

    fn calc_byte(&self, crc: u16, b: u8) -> u16 {
        let mut crc_new = crc ^ ((b as u32) << 8) as u16;
        for _i in 0..8 {
            if crc_new & 0x8000 == 0x8000 {
                crc_new = crc_new << 1 ^ 0x1021;
            } else {
                crc_new = crc_new << 1;
            }
        }
        return crc_new & 0xffff;
    }

    /* crc 16 */
    fn crc_calc(&self) -> u16{
        let mut crc_16: u16 = 0;
        if self.rx_frame_payload_bytes == 0 {
            return 0;
        }
        let slice: &[u8; MAX_LENGTH] = &self.rx_frame_buffer;
        for i in 0..self.rx_frame_payload_bytes {
            crc_16 = self.calc_byte(crc_16, slice[i as usize]);
        }
        return crc_16;
    }

    fn crc_check(&self) -> Result<u16, u16> {
        let crc_16 = self.crc_calc();
        if crc_16 != self.rx_frame_fcs {
            return Err(crc_16);
        }
        Ok(crc_16)
    }

    fn handle_msg(&mut self) {
        match self.crc_check() {
            Ok(_) => {
                self.rx_msg_is_ready = true;
            },
            Err(err) => {
                error!("rx_frame_fcs: {}", self.rx_frame_fcs);
                error!("crc 16: {}", err);
            },
        }
    }

    pub fn get_msg(&self) -> Result<String, ()> {
        if self.rx_msg_is_ready {
            let slice: &[u8; MAX_LENGTH] = &self.rx_frame_buffer;
            let r = String::from_utf8(slice[0..self.rx_frame_payload_bytes as usize].to_vec());
            match r {
                Ok(string) => return Ok(string),
                Err(_) => return Err(()),
            }
        } else {
            return Err(());
        }
    }
}
