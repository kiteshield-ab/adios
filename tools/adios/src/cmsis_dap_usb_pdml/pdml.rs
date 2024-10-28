use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Pdml {
    pub packet: Vec<Packet>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Packet {
    pub proto: Vec<Proto>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Proto {
    #[serde(rename = "@name")]
    pub name: String,
    pub field: Vec<Field>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Field {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@show")]
    pub show: String,
    #[serde(rename = "@showname")]
    pub showname: Option<String>,
    #[serde(rename = "@value")]
    pub value: Option<String>,
    #[serde(rename = "@size")]
    pub size: String,
    #[serde(rename = "@hide")]
    pub hide: Option<String>,
    #[serde(rename = "@unmaskedvalue")]
    pub unmaskedvalue: Option<String>,
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(default)]
    pub field: Vec<Field>,
}
