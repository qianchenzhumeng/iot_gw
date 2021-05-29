
use std::fmt;
use log::error;

const START_OF_FRAM: u8 = 0x7E;

const MAX_LENGTH: usize = 255;

#[derive(Debug)]
enum MechainState {
    SearchingForFlag,
    ReceivingLength,
    ReceivingPayload,
    ReceivingFcs,
}

#[derive(Debug)]
enum FcsState {
    Start,
    End,
}

#[derive(Debug)]
enum HdtpStatus {
    Ok,
    RxNotDone,     // 接收尚未完成
    FrameError,     // 帧格式错误
    FcsError,
}

#[derive(Debug)]
pub struct Hdtp {
    rx_frame_state: MechainState,
    rx_frame_length: u8,
    rx_frame_buffer: [u8; MAX_LENGTH],
    rx_frame_payload_bytes: u8,
    rx_fcs_state: FcsState,
    rx_frame_fcs: u16,
    rx_expected_fcs: u16,
    status: HdtpStatus,
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
        write!(f, "rx_frame_state: {}, rx_frame_length: {}, ", self.rx_frame_state, self.rx_frame_length)?;
        
        write!(f, "rx_frame_buffer: [")?;
        for i in 0..(MAX_LENGTH - 1) {
            write!(f, "{}, ", slice[i as usize])?;
        }
        write!(f, "{}], ", slice[(MAX_LENGTH - 1)as usize])?;
        write!(f, "rx_frame_payload_bytes: {}, ", self.rx_frame_payload_bytes)?;
        write!(f, "rx_fcs_state: {}, ", self.rx_fcs_state)?;
        write!(f, "rx_frame_fcs: 0x{:X}, ", self.rx_frame_fcs)?;
        write!(f, "rx_expected_fcs: 0x{:X}, ", self.rx_expected_fcs)?;
        write!(f, "status: {:#?}, ", self.status)?;
        write!(f, "}}")?;
        Ok(())
    }
}

impl Hdtp {
    pub fn new() -> Self {
        Hdtp {
            // 初始化状态机
            rx_frame_state: MechainState::SearchingForFlag,
            rx_frame_length: 0,
            rx_frame_buffer: [0_u8; MAX_LENGTH],
            rx_frame_payload_bytes: 0,
            rx_fcs_state: FcsState::Start,
            rx_frame_fcs: 0,
            rx_expected_fcs: 0,
            status: HdtpStatus::RxNotDone,
        }
    }

    pub fn input(&mut self, c: u8) {
        match self.rx_frame_state {
            MechainState::SearchingForFlag => {
                self.rx_frame_payload_bytes = 0;
                self.rx_frame_length = 0;
                self.rx_frame_fcs = 0;
                // 如果是开始标志，将状态迁移到“ReceivingLength”
                if c == START_OF_FRAM {
                    self.rx_frame_state = MechainState::ReceivingLength;
                }
            },
            MechainState::ReceivingLength => {
                if c == 0 {
                    // 长度错误，将状态迁移到“SearchingForFlag”
                    self.rx_frame_state = MechainState::SearchingForFlag;
                    self.status = HdtpStatus::FrameError;
                } else {
                    self.rx_frame_length = c;
                // 将状态迁移到“ReceivingPayload”
                self.rx_frame_state = MechainState::ReceivingPayload;
                }
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
                        match self.handle_msg() {
                            Ok(_) => {
                                self.status = HdtpStatus::Ok;
                            },
                            Err(status) => {
                                self.status = status;
                            },
                        }
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

    /* CRC-16/XModem */
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

    fn crc_check(& mut self) -> Result<u16, u16> {
        self.rx_expected_fcs = self.crc_calc();
        if self.rx_expected_fcs != self.rx_frame_fcs {
            return Err(self.rx_expected_fcs);
        }
        Ok(self.rx_expected_fcs)
    }

    fn handle_msg(&mut self) -> Result<(), HdtpStatus> {
        match self.crc_check() {
            Ok(_) => {
                Ok(())
            },
            Err(err) => {
                error!("rx_frame_fcs: {}", self.rx_frame_fcs);
                error!("crc 16: {}", err);
                Err(HdtpStatus::FcsError)
            },
        }
    }

    pub fn get_msg(&self) -> Result<String, ()> {
        match self.status {
            HdtpStatus::Ok => {
                let slice: &[u8; MAX_LENGTH] = &self.rx_frame_buffer;
                let r = String::from_utf8(slice[0..self.rx_frame_payload_bytes as usize].to_vec());
                match r {
                    Ok(string) => return Ok(string),
                    Err(_) => return Err(()),
                }
            },
            _ => return Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    /// 使用 payload 为 255 个 '0' 的数据集进行测试
    fn test1() {
        let s = "0000000000000000000000000000000000000000000000000000000000000000\
                 0000000000000000000000000000000000000000000000000000000000000000\
                 0000000000000000000000000000000000000000000000000000000000000000\
                 000000000000000000000000000000000000000000000000000000000000000";
        let mut hdtp = crate::Hdtp::new();
        let v_data = [0x7E_u8, 255,
            /* 255 字节的数据 */
            0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
            0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
            0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
            0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
            0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
            0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
            0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
            0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
            0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
            0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
            0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
            0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
            0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
            0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
            0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
            0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
            /* 2 字节 CRC */
            0xEB, 0x1D
        ];
        for c in &v_data {
            hdtp.input(*c);
        }
        match hdtp.get_msg() {
            Ok(string) => {
                assert_eq!(s, string);
            },
            Err(_) => panic!("htdp.status: {:#?}", hdtp.status),
        }
    }

    #[test]
    /// 使用 payload 为 "0123" 的数据集进行测试
    fn test2() {
        let s = "0123";
        let mut hdtp = crate::Hdtp::new();
        let v_data = [0x7E_u8, 4,
            0x30, 0x31, 0x32, 0x33,
            /* 2 字节 CRC */
            0xBB, 0xBB
        ];
        for c in &v_data {
            hdtp.input(*c);
        }
        match hdtp.get_msg() {
            Ok(string) => {
                assert_eq!(s, string);
            },
            Err(_) => panic!("status: {:#?}, fcs: 0x{:X}(expected: 0x{:X}).", hdtp.status, hdtp.rx_frame_fcs, hdtp.rx_expected_fcs),
        }
    }

    #[test]
    /// 使用 payload 为 "0123" 的数据集进行测试，CRC 错误
    fn test3() {
        let mut hdtp = crate::Hdtp::new();
        let v_data = [0x7E_u8, 4,
            0x30, 0x31, 0x32, 0x33,
            /* 2 字节 CRC */
            0, 0    // 0xBB, 0xBB
        ];
        for c in &v_data {
            hdtp.input(*c);
        }
        match hdtp.get_msg() {
            Ok(_) => panic!("{:#?}", hdtp),
            Err(_) => {},
        }
    }

    #[test]
    /// 数据帧长度错误（0）
    fn test4() {
        let mut hdtp = crate::Hdtp::new();
        let v_data = [0x7E_u8, 0,
            0x30, 0x31, 0x32, 0x33,
            /* 2 字节 CRC */
            0, 0 //0xBB, 0xBB
        ];
        for c in &v_data {
            hdtp.input(*c);
        }
        match hdtp.get_msg() {
            Ok(_) => panic!("{:#?}", hdtp),
            Err(_) => {},
        }
    }

    #[test]
    fn test5() {
        let mut hdtp = crate::Hdtp::new();
        let v_data = [0x7E_u8, 0x7E,
            0x30, 0x31, 0x32, 0x33,
            /* 2 字节 CRC */
            0xBB, 0xBB
        ];
        for c in &v_data {
            hdtp.input(*c);
        }
        match hdtp.get_msg() {
            Ok(_) => {},
            //Err(_) => panic!("status: {:#?}, fcs: 0x{:X}(expected: 0x{:X}).", hdtp.status, hdtp.rx_frame_fcs, hdtp.rx_expected_fcs),
            Err(_) => {
                match hdtp.status {
                    crate::HdtpStatus::RxNotDone => {},
                    _ => panic!("{:#?}", hdtp),
                }
            },
        }
    }
}
