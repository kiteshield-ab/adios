#![allow(warnings)]

use std::fmt::{Debug, Display};
use std::ops::DerefMut;
use std::{collections::HashMap, ops::Deref};

use svd_rs::PeripheralInfo;
use svd_rs::RegisterInfo as InnerRegisterInfo;
use svd_rs::{BitRange, EnumeratedValue};
use svd_rs::{FieldInfo, PeripheralInfoBuilder, ValidateLevel};

pub struct Database {
    pub regs: HashMap<u64, RegisterInfo>,
}

impl Database {
    pub fn new() -> Self {
        Self {
            regs: Default::default(),
        }
    }

    pub fn get_register(&self, address: u64) -> Option<&RegisterInfo> {
        self.regs.get(&address)
    }

    pub fn from_svd(device: svd_rs::Device) -> Self {
        let mut db = Self::new();
        db.extend_with_svd(device);
        db
    }

    pub fn extend_with_svd(&mut self, device: svd_rs::Device) {
        let device = Self::expand_derived_from_attribute(device);
        for peripheral in device.peripherals.into_iter() {
            let peripheral = peripheral.deref();
            let base_address = peripheral.base_address;
            for register in peripheral.registers() {
                let register = register.deref();
                let address = base_address + register.address_offset as u64;
                let register_desc = RegisterInfo {
                    address,
                    device_name: device.name.clone(),
                    peripheral_name: peripheral.name.clone(),
                    cluster_name: None,
                    inner: register.clone(),
                };
                let identifier = register_desc.identifier();
                match self.regs.insert(address, register_desc) {
                    Some(previous) => {
                        log::info!(
                            "Address collision: [{}] overwrites [{}]",
                            identifier,
                            previous.identifier(),
                        );
                    }
                    None => {}
                }
            }
            for cluster in peripheral.clusters() {
                let cluster = cluster.deref();
                let base_address = base_address + cluster.address_offset as u64;
                for register in cluster.registers() {
                    let register = register.deref();
                    let address = base_address + register.address_offset as u64;
                    let register_desc = RegisterInfo {
                        address,
                        device_name: device.name.clone(),
                        peripheral_name: peripheral.name.clone(),
                        cluster_name: Some(cluster.name.clone()),
                        inner: register.clone(),
                    };
                    let identifier = register_desc.identifier();
                    match self.regs.insert(address, register_desc) {
                        Some(previous) => {
                            log::info!(
                                "Address collision: [{}] overwrites [{}]",
                                identifier,
                                previous.identifier(),
                            );
                        }
                        None => {}
                    }
                }
            }
        }
    }

    fn expand_derived_from_attribute(mut device: svd_rs::Device) -> svd_rs::Device {
        let device_copy = device.clone();
        for peripheral in device
            .peripherals
            .iter_mut()
            .filter(|v| v.derived_from.is_some())
        {
            let peripheral = peripheral.deref_mut();
            let derived_from = peripheral.derived_from.as_ref().unwrap();
            let Some(derived_from_peripheral) = device_copy.get_peripheral(derived_from) else {
                log::warn!(
                    "{} derived from {} but the latter does not exist? Skipping",
                    &peripheral.name,
                    derived_from
                );
                continue;
            };
            let peripheral_as_builder =
                PeripheralInfoBuilder::from(peripheral.clone()).derived_from(None);
            let mut derived_from_peripheral = derived_from_peripheral.deref().clone();
            derived_from_peripheral
                .modify_from(peripheral_as_builder, Default::default())
                .unwrap();
            peripheral
                .modify_from(derived_from_peripheral.into(), Default::default())
                .unwrap();
            peripheral.derived_from = None;
        }
        if device
            .peripherals
            .iter()
            .filter(|v| v.derived_from.is_some())
            .count()
            > 0
        {
            log::warn!("Multi-pass derivedFrom peripherals expansion is not implemented, YMMV.");
        }
        device
    }
}

#[derive(Clone)]
pub struct RegisterInfo {
    address: u64,
    device_name: String,
    peripheral_name: String,
    /// Name of the cluster if it belongs to one
    cluster_name: Option<String>,
    inner: InnerRegisterInfo,
}

impl RegisterInfo {
    pub fn identifier(&self) -> String {
        let name = match &self.cluster_name {
            Some(cluster_name) => &format!("{}.{}", cluster_name, self.inner.name),
            None => &self.inner.name,
        };
        format!("{}.{}.{}", self.device_name, self.peripheral_name, name,)
    }
}

impl RegisterInfo {
    pub fn decode_value(&self, value: u64) -> Register {
        let mut fields = Vec::new();
        'fields: for field in self.inner.fields() {
            let field = field.deref();
            let BitRange { offset, width, .. } = field.bit_range;
            let field_value = to_field_value(value, offset, width);
            // https://arm-software.github.io/CMSIS_5/SVD/html/elem_registers.html#elem_enumeratedValue
            //
            // If enumeratedValue::value exists and matches the field value, push and move on to another field
            // If enumeratedValue::value exists but does not match the field value, keep looking (continue loop)
            // If enumeratedValue::value does not exist but variant is default (catch-all), save it for later
            // Other states are invalid
            let mut catch_all_variant = None;
            // I don't know if there is any point to this double-nested structure. Flattening.
            'variants: for variant in field
                .enumerated_values
                .clone()
                .into_iter()
                .map(|v| v.values)
                .flatten()
            {
                match variant.value {
                    Some(variant_value) if variant_value == field_value => {
                        fields.push(Field {
                            info: field.clone(),
                            parent_info: self.clone(),
                            value: field_value,
                            variant: Some(variant),
                        });
                        continue 'fields;
                    }
                    Some(_) => {}
                    None if variant.is_default() => {
                        catch_all_variant = Some(variant);
                    }
                    None => {
                        log::error!(
                            "Enumerated value ({}) has no value and is not default?",
                            &variant.name
                        );
                    }
                }
            }
            let field = match catch_all_variant {
                Some(catch_all_variant) => Field {
                    info: field.clone(),
                    parent_info: self.clone(),
                    value: field_value,
                    variant: Some(catch_all_variant),
                },
                None => Field {
                    info: field.clone(),
                    parent_info: self.clone(),
                    value: field_value,
                    variant: None,
                },
            };
            fields.push(field);
        }
        // Sort fields by their offset
        fields.sort_by_key(|v| v.info.bit_offset());
        Register {
            info: self.clone(),
            value,
            fields,
        }
    }
}

fn to_field_value(value: u64, offset: u32, width: u32) -> u64 {
    let mask = !(u64::MAX >> width << width);
    (value >> offset) & mask
}

pub struct Register {
    info: RegisterInfo,
    value: u64,
    // Should be sorted by their bit offset
    fields: Vec<Field>,
}

// It exists to nicely compose with `RegisterDiff` Register::diff(None, register)
pub struct RegisterDiffFromNothing<'a>(&'a Register);

impl<'a> Display for RegisterDiffFromNothing<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "0x???????? → {:#010x}", self.0.value)?;
        for field in self.0.fields.iter() {
            write!(f, "  {} : 0x? → {:#0x}", field.info.name, field.value)?;
            match &field.variant {
                Some(variant) => writeln!(f, " / ? → {}", variant.name)?,
                None => writeln!(f)?,
            }
        }
        Ok(())
    }
}

impl Debug for Register {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&self.info.identifier())
            .field("value", &self.value)
            .field("fields", &self.fields)
            .finish()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct WrongRegister;

impl Register {
    pub fn diff_from_nothing<'a>(&'a self) -> RegisterDiffFromNothing<'a> {
        RegisterDiffFromNothing(self)
    }
    pub fn diff(old: &Self, new: &Self) -> Result<Option<RegisterDiff>, WrongRegister> {
        if old.info.identifier() != new.info.identifier() {
            return Err(WrongRegister);
        }
        if old.value == new.value {
            return Ok(None);
        }

        let fields = old
            .fields
            .iter()
            .zip(new.fields.iter())
            .filter_map(|(old, new)| {
                assert_eq!(
                    old.info.name, new.info.name,
                    "Fields should be in the same order"
                );
                if old.value != new.value {
                    Some(FieldDiff {
                        info: old.info.clone(),
                        old: old.value,
                        new: new.value,
                        old_variant: old.variant.clone(),
                        new_variant: new.variant.clone(),
                    })
                } else {
                    None
                }
            })
            .collect();
        Ok(Some(RegisterDiff {
            old: old.value,
            new: new.value,
            fields,
        }))
    }
}

pub struct RegisterDiff {
    old: u64,
    new: u64,
    fields: Vec<FieldDiff>,
}

impl Display for RegisterDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{:#010x} → {:#010x}", self.old, self.new)?;
        for field in self.fields.iter() {
            write!(
                f,
                "  {} : {:#0x} → {:#0x}",
                field.info.name, field.old, field.new
            )?;
            match (&field.old_variant, &field.new_variant) {
                (Some(old), Some(new)) => writeln!(f, " / {} → {}", old.name, new.name)?,
                (None, None) => writeln!(f)?,
                (None, Some(new)) => writeln!(f, " / ?! → {}", new.name)?,
                (Some(old), None) => writeln!(f, " / {} → ?!", old.name)?,
            }
        }
        Ok(())
    }
}

pub struct FieldDiff {
    info: FieldInfo,
    old: u64,
    new: u64,
    old_variant: Option<EnumeratedValue>,
    new_variant: Option<EnumeratedValue>,
}

pub struct Field {
    pub parent_info: RegisterInfo,
    pub info: FieldInfo,
    pub value: u64,
    pub variant: Option<EnumeratedValue>,
}

impl Debug for Field {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut f = f.debug_struct(&self.info.name);
        f.field("value", &self.value);
        if let Some(variant) = &self.variant {
            f.field("variant", &variant.name);
        }
        f.finish()
    }
}

#[cfg(test)]
mod tests {
    use svd_rs::MaybeArray::Single;
    use svd_rs::Name;
    use svd_rs::ValidateLevel::Weak;

    use super::*;

    #[test]
    fn derived_from_attribute_expanding() {
        let device = svd_parser::parse(
            r#"
<device>
    <name>MIMXRT1189_cm33(S)</name>
    <peripherals>
    <peripheral>
      <name>XCACHE_PC</name>
      <description>XCACHE</description>
      <groupName>XCACHE</groupName>
      <headerStructName>XCACHE</headerStructName>
      <baseAddress>0x54400000</baseAddress>
      <addressBlock>
        <offset>0</offset>
        <size>0x10</size>
        <usage>registers</usage>
      </addressBlock>
      <registers>
        <register>
          <name>CCR</name>
          <description>Cache Control</description>
          <addressOffset>0</addressOffset>
          <size>32</size>
          <access>read-write</access>
          <resetValue>0</resetValue>
          <resetMask>0xFFFFFFFF</resetMask>
          <fields>
            <field>
              <name>ENCACHE</name>
              <description>Cache Enable</description>
              <bitOffset>0</bitOffset>
              <bitWidth>1</bitWidth>
              <access>read-write</access>
              <enumeratedValues>
                <enumeratedValue>
                  <name>disabled</name>
                  <description>Disable</description>
                  <value>0</value>
                </enumeratedValue>
                <enumeratedValue>
                  <name>enabled</name>
                  <description>Enable</description>
                  <value>0x1</value>
                </enumeratedValue>
              </enumeratedValues>
            </field>
            <field>
              <name>FRCWT</name>
              <description>Force Write Through Mode</description>
              <bitOffset>2</bitOffset>
              <bitWidth>1</bitWidth>
              <access>read-write</access>
              <enumeratedValues>
                <enumeratedValue>
                  <name>FRCWT_0</name>
                  <description>Does not force</description>
                  <value>0</value>
                </enumeratedValue>
                <enumeratedValue>
                  <name>FRCWT_1</name>
                  <description>Force</description>
                  <value>0x1</value>
                </enumeratedValue>
              </enumeratedValues>
            </field>
            <field>
              <name>FRCNOALLC</name>
              <description>Forces No Allocation on Cache Misses</description>
              <bitOffset>3</bitOffset>
              <bitWidth>1</bitWidth>
              <access>read-write</access>
              <enumeratedValues>
                <enumeratedValue>
                  <name>FRCNOALLC_0</name>
                  <description>Allocation on cache misses</description>
                  <value>0</value>
                </enumeratedValue>
                <enumeratedValue>
                  <name>FRCNOALLC_1</name>
                  <description>Forces no allocation on cache misses (must also have FRCWT asserted)</description>
                  <value>0x1</value>
                </enumeratedValue>
              </enumeratedValues>
            </field>
            <field>
              <name>INVW0</name>
              <description>Invalidate Way 0</description>
              <bitOffset>24</bitOffset>
              <bitWidth>1</bitWidth>
              <access>read-write</access>
              <enumeratedValues>
                <enumeratedValue>
                  <name>no_operation</name>
                  <description>No operation</description>
                  <value>0</value>
                </enumeratedValue>
                <enumeratedValue>
                  <name>invw0</name>
                  <description>When you write 1 to GO, invalidates all lines in way 0.</description>
                  <value>0x1</value>
                </enumeratedValue>
              </enumeratedValues>
            </field>
            <field>
              <name>PUSHW0</name>
              <description>Push Way 0</description>
              <bitOffset>25</bitOffset>
              <bitWidth>1</bitWidth>
              <access>read-write</access>
              <enumeratedValues>
                <enumeratedValue>
                  <name>no_operation</name>
                  <description>No operation</description>
                  <value>0</value>
                </enumeratedValue>
                <enumeratedValue>
                  <name>pushw0</name>
                  <description>When you write 1 to GO, push all modified lines in way 0</description>
                  <value>0x1</value>
                </enumeratedValue>
              </enumeratedValues>
            </field>
            <field>
              <name>INVW1</name>
              <description>Invalidate Way 1</description>
              <bitOffset>26</bitOffset>
              <bitWidth>1</bitWidth>
              <access>read-write</access>
              <enumeratedValues>
                <enumeratedValue>
                  <name>no_operation</name>
                  <description>No operation</description>
                  <value>0</value>
                </enumeratedValue>
                <enumeratedValue>
                  <name>invw1</name>
                  <description>When you write 1 to GO, invalidates all lines in way 1</description>
                  <value>0x1</value>
                </enumeratedValue>
              </enumeratedValues>
            </field>
            <field>
              <name>PUSHW1</name>
              <description>Push Way 1</description>
              <bitOffset>27</bitOffset>
              <bitWidth>1</bitWidth>
              <access>read-write</access>
              <enumeratedValues>
                <enumeratedValue>
                  <name>no_operation</name>
                  <description>No operation</description>
                  <value>0</value>
                </enumeratedValue>
                <enumeratedValue>
                  <name>pushw1</name>
                  <description>When you write 1 to GO, push all modified lines in way 1</description>
                  <value>0x1</value>
                </enumeratedValue>
              </enumeratedValues>
            </field>
            <field>
              <name>GO</name>
              <description>Initiate Cache Command</description>
              <bitOffset>31</bitOffset>
              <bitWidth>1</bitWidth>
              <access>read-write</access>
              <enumeratedValues>
                <enumeratedValue>
                  <name>no_effect</name>
                  <description>Write: no effect. Read: no cache command active</description>
                  <value>0</value>
                </enumeratedValue>
                <enumeratedValue>
                  <name>init_cmd</name>
                  <description>Write: initiates command; Read: cache command active</description>
                  <value>0x1</value>
                </enumeratedValue>
              </enumeratedValues>
            </field>
          </fields>
        </register>
        <register>
          <name>CLCR</name>
          <description>Cache Line Control</description>
          <addressOffset>0x4</addressOffset>
          <size>32</size>
          <access>read-write</access>
          <resetValue>0</resetValue>
          <resetMask>0xFFFFFFFF</resetMask>
          <fields>
            <field>
              <name>LGO</name>
              <description>Initiate Cache Line Command</description>
              <bitOffset>0</bitOffset>
              <bitWidth>1</bitWidth>
              <access>read-write</access>
              <enumeratedValues>
                <enumeratedValue>
                  <name>no_effect</name>
                  <description>Write: no effect. Read: no line command active.</description>
                  <value>0</value>
                </enumeratedValue>
                <enumeratedValue>
                  <name>init_cmd</name>
                  <description>Write: initiate line command. Read: line command active.</description>
                  <value>0x1</value>
                </enumeratedValue>
              </enumeratedValues>
            </field>
            <field>
              <name>CACHEADDR</name>
              <description>Cache Address</description>
              <bitOffset>2</bitOffset>
              <bitWidth>12</bitWidth>
              <access>read-write</access>
            </field>
            <field>
              <name>WSEL</name>
              <description>Way Select</description>
              <bitOffset>14</bitOffset>
              <bitWidth>1</bitWidth>
              <access>read-write</access>
              <enumeratedValues>
                <enumeratedValue>
                  <name>way0</name>
                  <description>Way 0</description>
                  <value>0</value>
                </enumeratedValue>
                <enumeratedValue>
                  <name>way1</name>
                  <description>Way 1</description>
                  <value>0x1</value>
                </enumeratedValue>
              </enumeratedValues>
            </field>
            <field>
              <name>TDSEL</name>
              <description>Tag or Data Select</description>
              <bitOffset>16</bitOffset>
              <bitWidth>1</bitWidth>
              <access>read-write</access>
              <enumeratedValues>
                <enumeratedValue>
                  <name>data</name>
                  <description>Data</description>
                  <value>0</value>
                </enumeratedValue>
                <enumeratedValue>
                  <name>tag</name>
                  <description>Tag</description>
                  <value>0x1</value>
                </enumeratedValue>
              </enumeratedValues>
            </field>
            <field>
              <name>LCIVB</name>
              <description>Line Command Initial Valid</description>
              <bitOffset>20</bitOffset>
              <bitWidth>1</bitWidth>
              <access>read-write</access>
            </field>
            <field>
              <name>LCIMB</name>
              <description>Line Command Initial Modified</description>
              <bitOffset>21</bitOffset>
              <bitWidth>1</bitWidth>
              <access>read-write</access>
            </field>
            <field>
              <name>LCWAY</name>
              <description>Line Command Way</description>
              <bitOffset>22</bitOffset>
              <bitWidth>1</bitWidth>
              <access>read-write</access>
            </field>
            <field>
              <name>LCMD</name>
              <description>Line Command</description>
              <bitOffset>24</bitOffset>
              <bitWidth>2</bitWidth>
              <access>read-write</access>
              <enumeratedValues>
                <enumeratedValue>
                  <name>search_rw</name>
                  <description>Search and read or write</description>
                  <value>0</value>
                </enumeratedValue>
                <enumeratedValue>
                  <name>invalidate</name>
                  <description>Invalidate</description>
                  <value>0x1</value>
                </enumeratedValue>
                <enumeratedValue>
                  <name>push</name>
                  <description>Push</description>
                  <value>0x2</value>
                </enumeratedValue>
                <enumeratedValue>
                  <name>clear</name>
                  <description>Clear</description>
                  <value>0x3</value>
                </enumeratedValue>
              </enumeratedValues>
            </field>
            <field>
              <name>LADSEL</name>
              <description>Line Address Select</description>
              <bitOffset>26</bitOffset>
              <bitWidth>1</bitWidth>
              <access>read-write</access>
              <enumeratedValues>
                <enumeratedValue>
                  <name>cache_addr</name>
                  <description>Cache address</description>
                  <value>0</value>
                </enumeratedValue>
                <enumeratedValue>
                  <name>phys_addr</name>
                  <description>Physical address</description>
                  <value>0x1</value>
                </enumeratedValue>
              </enumeratedValues>
            </field>
            <field>
              <name>LACC</name>
              <description>Line Access Type</description>
              <bitOffset>27</bitOffset>
              <bitWidth>1</bitWidth>
              <access>read-write</access>
              <enumeratedValues>
                <enumeratedValue>
                  <name>read</name>
                  <description>Read</description>
                  <value>0</value>
                </enumeratedValue>
                <enumeratedValue>
                  <name>write</name>
                  <description>Write</description>
                  <value>0x1</value>
                </enumeratedValue>
              </enumeratedValues>
            </field>
          </fields>
        </register>
        <register>
          <name>CSAR</name>
          <description>Cache Search Address</description>
          <addressOffset>0x8</addressOffset>
          <size>32</size>
          <access>read-write</access>
          <resetValue>0</resetValue>
          <resetMask>0xFFFFFFFF</resetMask>
          <fields>
            <field>
              <name>LGO</name>
              <description>Initiate Cache Line Command</description>
              <bitOffset>0</bitOffset>
              <bitWidth>1</bitWidth>
              <access>read-write</access>
              <enumeratedValues>
                <enumeratedValue>
                  <name>no_effect</name>
                  <description>Write: no effect. Read: no line command active.</description>
                  <value>0</value>
                </enumeratedValue>
                <enumeratedValue>
                  <name>init_cmd</name>
                  <description>Write: initiate line command. Read: line command active.</description>
                  <value>0x1</value>
                </enumeratedValue>
              </enumeratedValues>
            </field>
            <field>
              <name>PHYADDR</name>
              <description>Physical Address</description>
              <bitOffset>2</bitOffset>
              <bitWidth>30</bitWidth>
              <access>read-write</access>
            </field>
          </fields>
        </register>
        <register>
          <name>CCVR</name>
          <description>Cache Read/Write Value</description>
          <addressOffset>0xC</addressOffset>
          <size>32</size>
          <access>read-write</access>
          <resetValue>0</resetValue>
          <resetMask>0xFFFFFFFF</resetMask>
          <fields>
            <field>
              <name>DATA</name>
              <description>Cache Read/Write Data</description>
              <bitOffset>0</bitOffset>
              <bitWidth>32</bitWidth>
              <access>read-write</access>
            </field>
          </fields>
        </register>
      </registers>
    </peripheral>
    <peripheral derivedFrom="XCACHE_PC">
      <name>XCACHE_PS</name>
      <description>XCACHE</description>
      <groupName>XCACHE</groupName>
      <baseAddress>0x54400800</baseAddress>
      <addressBlock>
        <offset>0</offset>
        <size>0x10</size>
        <usage>registers</usage>
      </addressBlock>
    </peripheral>
    </peripherals>
</device>
            "#
        ).unwrap();
        let xcache_pc_before = device.get_peripheral("XCACHE_PC").unwrap().deref().clone();
        let xcache_ps_before = device.get_peripheral("XCACHE_PS").unwrap().deref().clone();
        let device = Database::expand_derived_from_attribute(device);
        let xcache_ps_after = device.get_peripheral("XCACHE_PS").unwrap().deref();
        assert_eq!(xcache_ps_before.registers.iter().count(), 0);
        assert_eq!(xcache_ps_after.registers().collect::<Vec<_>>(), xcache_pc_before.registers().collect::<Vec<_>>());
        assert!(xcache_ps_before.derived_from.is_some());
        assert!(xcache_ps_after.derived_from.is_none());
    }

    #[test]
    fn decoding() {
        let test_samples = [
            ("Test field 1", 0xf),
            ("Test field 2", 0xe),
            ("Test field 3", 0xd),
            ("Test field 4", 0xc),
            ("Test field 5", 0xb),
            ("Test field 6", 0xa),
            ("Test field 7", 0x9),
            ("Test field 8", 0x8),
        ];
        let fields = test_samples
            .iter()
            .enumerate()
            .map(|(i, (name, _))| {
                Single(
                    FieldInfo::builder()
                        .name(String::from(*name))
                        .bit_range(BitRange::from_offset_width(i as u32 * 4, 4))
                        .build(Weak)
                        .unwrap(),
                )
            })
            .collect();
        let svd = svd_rs::Device::builder()
            .name("Test device".into())
            .peripherals(vec![Single(
                PeripheralInfo::builder()
                    .name("Test peripheral".into())
                    .base_address(0xDEAD0000)
                    .registers(Some(vec![Single(
                        // TODO: Add some cluster tests maybe also?
                        InnerRegisterInfo::builder()
                            .name("Test register".into())
                            .address_offset(0x4)
                            .fields(Some(fields))
                            .build(Weak)
                            .unwrap(),
                    )
                    .into()]))
                    .build(Weak)
                    .unwrap(),
            )])
            .build(Weak)
            .unwrap();
        let db = Database::from_svd(svd);
        let register_info = db.get_register(0xDEAD0004).unwrap();
        let register = register_info.decode_value(0x89abcdef);
        for (field_name, expected_value) in test_samples {
            let actual_value = register
                .fields
                .iter()
                .find(|v| v.info.name == field_name)
                .unwrap()
                .value;
            assert_eq!(expected_value, actual_value);
        }
    }

    #[test]
    fn to_field_value() {
        let value = 0b00110101;
        for (result, offset, width) in [
            (0b0, 0, 0),
            (0b0, 2, 0),
            (0b0, 4, 0),
            (0b0, 6, 0),
            (0b0, 8, 0),
            (0b01, 0, 2),
            (0b01, 2, 2),
            (0b11, 4, 2),
            (0b00, 6, 2),
            (0b00, 8, 2),
            (0b0101, 0, 4),
            (0b1101, 2, 4),
            (0b0011, 4, 4),
            (0b0000, 6, 4),
            (0b0000, 8, 4),
            (0b110101, 0, 6),
            (0b001101, 2, 6),
            (0b000011, 4, 6),
            (0b000000, 6, 6),
            (0b000000, 8, 6),
            (0b00110101, 0, 8),
            (0b00001101, 2, 8),
            (0b00000011, 4, 8),
            (0b00000000, 6, 8),
            (0b00000000, 8, 8),
        ] {
            assert_eq!(result, super::to_field_value(value, offset, width));
        }
    }
}
