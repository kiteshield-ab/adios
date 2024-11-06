use regdoctor::RegisterInfo;

pub struct Database {
    inner: regdoctor::Database,
}

impl Database {
    const CSW_GENERIC: u64 = 0xFFFFFFFF00000000;
    const CSW_AMBA_AHB3: u64 = Self::CSW_GENERIC + 0x20;
    pub fn new() -> Self {
        let device = svd_parser::parse(include_str!("adi.svd")).unwrap();
        let inner = regdoctor::Database::from_svd(device);
        Self { inner }
    }

    pub fn ap_csw(&self, type_: CswType) -> RegisterInfo {
        let addr = match type_ {
            CswType::Generic => Self::CSW_GENERIC,
            CswType::AmbaAhb3 => Self::CSW_AMBA_AHB3,
        };
        self.inner.get_register(addr).unwrap().clone()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CswType {
    Generic,
    AmbaAhb3,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_does_not_crash() {
        let db = Database::new();
        let csw_reg_info = db.ap_csw(CswType::Generic);
        let csw_reg = csw_reg_info.decode_value(0x03000002);
        let csw_reg_1 = csw_reg_info.decode_value(0x04001002);
        let _ = regdoctor::Register::diff(&csw_reg, &csw_reg_1)
            .unwrap()
            .unwrap();
        let csw_reg_info = db.ap_csw(CswType::AmbaAhb3);
        let csw_reg = csw_reg_info.decode_value(0x03000002);
        let csw_reg_1 = csw_reg_info.decode_value(0x04001002);
        let _ = regdoctor::Register::diff(&csw_reg, &csw_reg_1)
            .unwrap()
            .unwrap();
    }
}
