use chrono::Local;

/// 参与 bindid 的最小契约：各模块给出自己的组合键
pub trait CompKey {
    fn get_composite_key(&self) -> String;
}

/// 入库契约：把一个设备映射到一行
pub trait ToRow {
    fn to_row(&self, bind_id: &str) -> ComponentRow;
}

/// 表行：字段与你给的表结构 1:1
#[derive(Default, Debug, Clone)]
#[allow(non_snake_case)]
pub struct ComponentRow {
    pub fd_CODE: Option<String>,
    pub fd_NAME: Option<String>,
    pub fd_SN: Option<String>,
    pub fd_TYPE: Option<String>,
    pub fd_COMPANY: Option<String>,
    pub fd_VOL: Option<String>,
    pub fd_VOL_REAL: Option<String>,
    pub fd_DEV_TYPE: Option<String>,
    pub fd_INFO_EX: [Option<String>; 10], // 1..=10
}

impl ComponentRow {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn code(mut self, v: impl Into<String>) -> Self {
        self.fd_CODE = Some(v.into());
        self
    }
    pub fn name(mut self, v: impl Into<String>) -> Self {
        self.fd_NAME = Some(v.into());
        self
    }
    pub fn sn(mut self, v: impl Into<String>) -> Self {
        self.fd_SN = Some(v.into());
        self
    }
    pub fn r#type(mut self, v: impl Into<String>) -> Self {
        self.fd_TYPE = Some(v.into());
        self
    }
    pub fn company(mut self, v: impl Into<String>) -> Self {
        self.fd_COMPANY = Some(v.into());
        self
    }
    pub fn vol(mut self, v: impl Into<String>) -> Self {
        self.fd_VOL = Some(v.into());
        self
    }
    pub fn vol_real(mut self, v: impl Into<String>) -> Self {
        self.fd_VOL_REAL = Some(v.into());
        self
    }
    pub fn dev_type(mut self, v: impl Into<String>) -> Self {
        self.fd_DEV_TYPE = Some(v.into());
        self
    }
    pub fn ex(mut self, idx: usize, v: impl Into<String>) -> Self {
        if (1..=10).contains(&idx) {
            self.fd_INFO_EX[idx - 1] = Some(v.into());
        }
        self
    }
    /// 若 fd_INFO_EX9（时间戳）未填，自动补
    pub fn ensure_timestamp(mut self) -> Self {
        if self.fd_INFO_EX[8].is_none() {
            self.fd_INFO_EX[8] = Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
        }
        self
    }
}
