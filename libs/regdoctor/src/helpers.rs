use std::ops::{Deref, DerefMut};

use svd_rs::PeripheralInfoBuilder;

pub fn expand_derived_from_attribute(mut device: svd_rs::Device) -> svd_rs::Device {
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
        let device = expand_derived_from_attribute(device);
        let xcache_ps_after = device.get_peripheral("XCACHE_PS").unwrap().deref();
        assert_eq!(xcache_ps_before.registers.iter().count(), 0);
        assert_eq!(
            xcache_ps_after.registers().collect::<Vec<_>>(),
            xcache_pc_before.registers().collect::<Vec<_>>()
        );
        assert!(xcache_ps_before.derived_from.is_some());
        assert!(xcache_ps_after.derived_from.is_none());
    }
}
